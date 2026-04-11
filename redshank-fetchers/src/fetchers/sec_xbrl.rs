//! SEC XBRL structured financial data fetcher.
//!
//! APIs:
//! - <https://data.sec.gov/api/xbrl/companyfacts/CIK{10-digit-cik}.json>
//! - <https://data.sec.gov/api/xbrl/companyconcept/CIK{cik}/us-gaap/{tag}.json>
//! - <https://data.sec.gov/api/xbrl/frames/us-gaap/Revenues/USD/CY2023.json>
//!
//! This fetcher focuses on the company facts endpoint and normalizes a core set
//! of financial metrics into period-based records.

use crate::domain::{FetchError, FetchOutput};
use crate::{build_client, write_ndjson};
use std::collections::BTreeMap;
use std::path::Path;

const COMPANY_FACTS_BASE: &str = "https://data.sec.gov/api/xbrl/companyfacts";

const REVENUE_TAGS: &[&str] = &["Revenues", "RevenueFromContractWithCustomerExcludingAssessedTax"];
const NET_INCOME_TAGS: &[&str] = &["NetIncomeLoss"];
const TOTAL_ASSETS_TAGS: &[&str] = &["Assets"];
const TOTAL_LIABILITIES_TAGS: &[&str] = &["Liabilities"];
const EQUITY_TAGS: &[&str] = &["StockholdersEquity", "StockholdersEquityIncludingPortionAttributableToNoncontrollingInterest"];
const CURRENT_ASSETS_TAGS: &[&str] = &["AssetsCurrent"];
const CURRENT_LIABILITIES_TAGS: &[&str] = &["LiabilitiesCurrent"];
const RELATED_PARTY_TAGS: &[&str] = &[
    "RelatedPartyTransactionAmount",
    "RelatedPartyTransactionsByRelatedParty",
    "RelatedPartyTransactionDescriptionAndTermsOfTransactionTextBlock",
];
const EXEC_COMP_NAME_TAGS: &[&str] = &[
    "ExecutiveOfficerName",
    "NamedExecutiveOfficerName",
];
const EXEC_COMP_TOTAL_TAGS: &[&str] = &[
    "ExecutiveOfficerTotalCompensation",
    "NamedExecutiveOfficerTotalCompensation",
    "SummaryCompensationTableTotal",
];

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq)]
pub struct FinancialMetrics {
    pub revenue: Option<f64>,
    pub net_income: Option<f64>,
    pub total_assets: Option<f64>,
    pub total_liabilities: Option<f64>,
    pub stockholders_equity: Option<f64>,
    pub current_assets: Option<f64>,
    pub current_liabilities: Option<f64>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq)]
pub struct FinancialRatios {
    pub current_ratio: Option<f64>,
    pub debt_to_equity: Option<f64>,
    pub profit_margin: Option<f64>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq)]
pub struct ExecutiveCompensationRecord {
    pub officer_name: String,
    pub title: Option<String>,
    pub total_compensation: Option<f64>,
    pub source_filing: Option<String>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq)]
pub struct RelatedPartyTransactionRecord {
    pub concept: String,
    pub value: String,
    pub period_end: Option<String>,
    pub source_filing: Option<String>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq)]
pub struct SecXbrlRecord {
    pub cik: String,
    pub ticker: Option<String>,
    pub company_name: String,
    pub period_end: String,
    pub period_type: String,
    pub financials: FinancialMetrics,
    pub ratios: FinancialRatios,
    pub source_filing: String,
    pub executive_compensation: Vec<ExecutiveCompensationRecord>,
    pub related_party_transactions: Vec<RelatedPartyTransactionRecord>,
}

#[derive(Debug, Clone, Default)]
struct PeriodAccumulator {
    revenue: Option<f64>,
    net_income: Option<f64>,
    total_assets: Option<f64>,
    total_liabilities: Option<f64>,
    stockholders_equity: Option<f64>,
    current_assets: Option<f64>,
    current_liabilities: Option<f64>,
    related_party_transactions: Vec<RelatedPartyTransactionRecord>,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
struct PeriodKey {
    end: String,
    form: String,
    accession: String,
}

#[derive(Debug, Clone, Copy)]
enum MetricField {
    Revenue,
    NetIncome,
    TotalAssets,
    TotalLiabilities,
    StockholdersEquity,
    CurrentAssets,
    CurrentLiabilities,
}

impl PeriodAccumulator {
    fn set_metric(&mut self, field: MetricField, value: f64) {
        match field {
            MetricField::Revenue => self.revenue = self.revenue.or(Some(value)),
            MetricField::NetIncome => self.net_income = self.net_income.or(Some(value)),
            MetricField::TotalAssets => self.total_assets = self.total_assets.or(Some(value)),
            MetricField::TotalLiabilities => {
                self.total_liabilities = self.total_liabilities.or(Some(value));
            }
            MetricField::StockholdersEquity => {
                self.stockholders_equity = self.stockholders_equity.or(Some(value));
            }
            MetricField::CurrentAssets => self.current_assets = self.current_assets.or(Some(value)),
            MetricField::CurrentLiabilities => {
                self.current_liabilities = self.current_liabilities.or(Some(value));
            }
        }
    }
}

fn company_facts_url(cik: &str) -> String {
    format!("{COMPANY_FACTS_BASE}/CIK{cik}.json")
}

fn period_type_for_form(form: &str) -> String {
    match form {
        "10-K" | "20-F" | "40-F" => "annual".to_string(),
        "10-Q" => "quarterly".to_string(),
        "8-K" => "event".to_string(),
        _ => "other".to_string(),
    }
}

fn as_f64(value: &serde_json::Value) -> Option<f64> {
    value
        .as_f64()
        .or_else(|| value.as_i64().map(|n| n as f64))
        .or_else(|| value.as_u64().map(|n| n as f64))
        .or_else(|| value.as_str().and_then(|s| s.parse::<f64>().ok()))
}

fn select_ticker(json: &serde_json::Value) -> Option<String> {
    json.get("tickers")
        .and_then(serde_json::Value::as_array)
        .and_then(|tickers| tickers.first())
        .and_then(serde_json::Value::as_str)
        .map(str::to_string)
}

fn metric_definitions() -> &'static [(&'static [ & 'static str], MetricField)] {
    &[
        (REVENUE_TAGS, MetricField::Revenue),
        (NET_INCOME_TAGS, MetricField::NetIncome),
        (TOTAL_ASSETS_TAGS, MetricField::TotalAssets),
        (TOTAL_LIABILITIES_TAGS, MetricField::TotalLiabilities),
        (EQUITY_TAGS, MetricField::StockholdersEquity),
        (CURRENT_ASSETS_TAGS, MetricField::CurrentAssets),
        (CURRENT_LIABILITIES_TAGS, MetricField::CurrentLiabilities),
    ]
}

fn calculate_ratios(financials: &FinancialMetrics) -> FinancialRatios {
    let current_ratio = match (financials.current_assets, financials.current_liabilities) {
        (Some(assets), Some(liabilities)) if liabilities != 0.0 => Some(assets / liabilities),
        _ => None,
    };

    let debt_to_equity = match (financials.total_liabilities, financials.stockholders_equity) {
        (Some(liabilities), Some(equity)) if equity != 0.0 => Some(liabilities / equity),
        _ => None,
    };

    let profit_margin = match (financials.net_income, financials.revenue) {
        (Some(net_income), Some(revenue)) if revenue != 0.0 => Some(net_income / revenue),
        _ => None,
    };

    FinancialRatios {
        current_ratio,
        debt_to_equity,
        profit_margin,
    }
}

fn push_related_party_transactions(
    facts: &serde_json::Value,
    grouped: &mut BTreeMap<PeriodKey, PeriodAccumulator>,
) {
    let Some(us_gaap) = facts.get("us-gaap").and_then(serde_json::Value::as_object) else {
        return;
    };

    for concept in RELATED_PARTY_TAGS {
        let Some(tag_obj) = us_gaap.get(*concept) else {
            continue;
        };
        let Some(units) = tag_obj.get("units").and_then(serde_json::Value::as_object) else {
            continue;
        };

        for unit_entries in units.values() {
            let Some(entries) = unit_entries.as_array() else {
                continue;
            };

            for entry in entries {
                let end = entry
                    .get("end")
                    .and_then(serde_json::Value::as_str)
                    .unwrap_or_default();
                let form = entry
                    .get("form")
                    .and_then(serde_json::Value::as_str)
                    .unwrap_or_default();
                let accession = entry
                    .get("accn")
                    .and_then(serde_json::Value::as_str)
                    .unwrap_or_default();

                if end.is_empty() || accession.is_empty() {
                    continue;
                }

                let value = entry
                    .get("val")
                    .map(|val| {
                        val.as_str()
                            .map(str::to_string)
                            .or_else(|| as_f64(val).map(|number| number.to_string()))
                            .unwrap_or_else(|| val.to_string())
                    })
                    .unwrap_or_default();

                let related = RelatedPartyTransactionRecord {
                    concept: (*concept).to_string(),
                    value,
                    period_end: Some(end.to_string()),
                    source_filing: Some(accession.to_string()),
                };

                grouped
                    .entry(PeriodKey {
                        end: end.to_string(),
                        form: form.to_string(),
                        accession: accession.to_string(),
                    })
                    .or_default()
                    .related_party_transactions
                    .push(related);
            }
        }
    }
}

/// Parse SEC company facts JSON into normalized period-based financial records.
#[must_use]
pub fn parse_company_facts(json: &serde_json::Value) -> Vec<SecXbrlRecord> {
    let company_name = json
        .get("entityName")
        .and_then(serde_json::Value::as_str)
        .unwrap_or_default()
        .to_string();
    let cik = json
        .get("cik")
        .and_then(serde_json::Value::as_str)
        .or_else(|| json.get("cik").and_then(serde_json::Value::as_u64).map(|n| Box::leak(format!("{n:010}").into_boxed_str()) as &str))
        .unwrap_or_default()
        .to_string();
    let ticker = select_ticker(json);

    let mut grouped: BTreeMap<PeriodKey, PeriodAccumulator> = BTreeMap::new();

    let Some(facts) = json.get("facts") else {
        return Vec::new();
    };
    let Some(us_gaap) = facts.get("us-gaap").and_then(serde_json::Value::as_object) else {
        return Vec::new();
    };

    for (tags, field) in metric_definitions() {
        for concept in *tags {
            let Some(tag_obj) = us_gaap.get(*concept) else {
                continue;
            };
            let Some(units) = tag_obj.get("units").and_then(serde_json::Value::as_object) else {
                continue;
            };

            for unit_entries in units.values() {
                let Some(entries) = unit_entries.as_array() else {
                    continue;
                };

                for entry in entries {
                    let Some(value) = entry.get("val").and_then(as_f64) else {
                        continue;
                    };
                    let end = entry
                        .get("end")
                        .and_then(serde_json::Value::as_str)
                        .unwrap_or_default();
                    let form = entry
                        .get("form")
                        .and_then(serde_json::Value::as_str)
                        .unwrap_or_default();
                    let accession = entry
                        .get("accn")
                        .and_then(serde_json::Value::as_str)
                        .unwrap_or_default();

                    if end.is_empty() || accession.is_empty() {
                        continue;
                    }

                    grouped
                        .entry(PeriodKey {
                            end: end.to_string(),
                            form: form.to_string(),
                            accession: accession.to_string(),
                        })
                        .or_default()
                        .set_metric(*field, value);
                }
            }
        }
    }

    push_related_party_transactions(facts, &mut grouped);

    grouped
        .into_iter()
        .map(|(key, values)| {
            let financials = FinancialMetrics {
                revenue: values.revenue,
                net_income: values.net_income,
                total_assets: values.total_assets,
                total_liabilities: values.total_liabilities,
                stockholders_equity: values.stockholders_equity,
                current_assets: values.current_assets,
                current_liabilities: values.current_liabilities,
            };

            SecXbrlRecord {
                cik: cik.clone(),
                ticker: ticker.clone(),
                company_name: company_name.clone(),
                period_end: key.end,
                period_type: period_type_for_form(&key.form),
                ratios: calculate_ratios(&financials),
                financials,
                source_filing: key.accession,
                executive_compensation: Vec::new(),
                related_party_transactions: values.related_party_transactions,
            }
        })
        .collect()
}

/// Parse simplified inline XBRL executive compensation data.
///
/// This helper accepts normalized JSON extracted from DEF 14A inline XBRL or a
/// similar transformation step.
#[must_use]
pub fn parse_inline_executive_compensation(
    json: &serde_json::Value,
) -> Vec<ExecutiveCompensationRecord> {
    if let Some(items) = json
        .get("executive_compensation")
        .and_then(serde_json::Value::as_array)
    {
        return items
            .iter()
            .filter_map(|item| {
                let officer_name = item
                    .get("officer_name")
                    .and_then(serde_json::Value::as_str)?
                    .to_string();
                Some(ExecutiveCompensationRecord {
                    officer_name,
                    title: item
                        .get("title")
                        .and_then(serde_json::Value::as_str)
                        .map(str::to_string),
                    total_compensation: item.get("total_compensation").and_then(as_f64),
                    source_filing: item
                        .get("source_filing")
                        .and_then(serde_json::Value::as_str)
                        .map(str::to_string),
                })
            })
            .collect();
    }

    let Some(facts) = json.get("facts") else {
        return Vec::new();
    };
    let Some(dei) = facts.get("dei").and_then(serde_json::Value::as_object) else {
        return Vec::new();
    };

    let officer_names: Vec<String> = EXEC_COMP_NAME_TAGS
        .iter()
        .filter_map(|tag| dei.get(*tag))
        .filter_map(|tag_obj| tag_obj.get("units"))
        .filter_map(serde_json::Value::as_object)
        .flat_map(|units| units.values())
        .filter_map(serde_json::Value::as_array)
        .flat_map(|entries| entries.iter())
        .filter_map(|entry| entry.get("val").and_then(serde_json::Value::as_str))
        .map(str::to_string)
        .collect();

    let totals: Vec<(Option<f64>, Option<String>)> = EXEC_COMP_TOTAL_TAGS
        .iter()
        .filter_map(|tag| dei.get(*tag))
        .filter_map(|tag_obj| tag_obj.get("units"))
        .filter_map(serde_json::Value::as_object)
        .flat_map(|units| units.values())
        .filter_map(serde_json::Value::as_array)
        .flat_map(|entries| entries.iter())
        .map(|entry| {
            (
                entry.get("val").and_then(as_f64),
                entry.get("accn").and_then(serde_json::Value::as_str).map(str::to_string),
            )
        })
        .collect();

    officer_names
        .iter()
        .enumerate()
        .map(|(index, name)| {
            let (total_compensation, source_filing) = totals
                .get(index)
                .cloned()
                .unwrap_or((None, None));
            ExecutiveCompensationRecord {
                officer_name: name.clone(),
                title: None,
                total_compensation,
                source_filing,
            }
        })
        .collect()
}

/// Fetch SEC company facts XBRL data for a CIK and write normalized NDJSON.
///
/// # Errors
///
/// Returns `Err` if the request fails, the API returns a non-success status, or
/// the response cannot be written.
pub async fn fetch_company_facts(cik: &str, output_dir: &Path) -> Result<FetchOutput, FetchError> {
    let client = build_client()?;
    let url = company_facts_url(cik);

    let resp = client.get(&url).send().await?;
    let status = resp.status();
    if !status.is_success() {
        let body = resp.text().await.unwrap_or_default();
        return Err(FetchError::ApiError {
            status: status.as_u16(),
            body,
        });
    }

    let json: serde_json::Value = resp.json().await?;
    let records = parse_company_facts(&json);
    let serialized: Result<Vec<serde_json::Value>, serde_json::Error> =
        records.iter().map(serde_json::to_value).collect();
    let serialized = serialized.map_err(|err| FetchError::Parse(err.to_string()))?;

    let output_path = output_dir.join("sec_xbrl.ndjson");
    let count = write_ndjson(&output_path, &serialized)?;

    Ok(FetchOutput {
        records_written: count,
        output_path,
        source_name: "sec-xbrl".to_string(),
        attribution: None,
    })
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::indexing_slicing)]
mod tests {
    use super::*;

    fn company_facts_fixture() -> serde_json::Value {
        serde_json::json!({
            "cik": "0000320193",
            "entityName": "Apple Inc.",
            "tickers": ["AAPL"],
            "facts": {
                "us-gaap": {
                    "Revenues": {
                        "units": {
                            "USD": [
                                {
                                    "val": 394328000000_u64,
                                    "accn": "0000320193-23-000106",
                                    "fy": 2023,
                                    "fp": "FY",
                                    "form": "10-K",
                                    "filed": "2023-11-03",
                                    "end": "2023-09-30"
                                },
                                {
                                    "val": 89500000000_u64,
                                    "accn": "0000320193-24-000081",
                                    "fy": 2024,
                                    "fp": "Q1",
                                    "form": "10-Q",
                                    "filed": "2024-02-02",
                                    "end": "2023-12-30"
                                }
                            ]
                        }
                    },
                    "NetIncomeLoss": {
                        "units": {
                            "USD": [
                                {
                                    "val": 96995000000_u64,
                                    "accn": "0000320193-23-000106",
                                    "fy": 2023,
                                    "fp": "FY",
                                    "form": "10-K",
                                    "filed": "2023-11-03",
                                    "end": "2023-09-30"
                                },
                                {
                                    "val": 23600000000_u64,
                                    "accn": "0000320193-24-000081",
                                    "fy": 2024,
                                    "fp": "Q1",
                                    "form": "10-Q",
                                    "filed": "2024-02-02",
                                    "end": "2023-12-30"
                                }
                            ]
                        }
                    },
                    "Assets": {
                        "units": {
                            "USD": [{
                                "val": 352583000000_u64,
                                "accn": "0000320193-23-000106",
                                "form": "10-K",
                                "end": "2023-09-30"
                            }]
                        }
                    },
                    "Liabilities": {
                        "units": {
                            "USD": [{
                                "val": 290437000000_u64,
                                "accn": "0000320193-23-000106",
                                "form": "10-K",
                                "end": "2023-09-30"
                            }]
                        }
                    },
                    "StockholdersEquity": {
                        "units": {
                            "USD": [{
                                "val": 62146000000_u64,
                                "accn": "0000320193-23-000106",
                                "form": "10-K",
                                "end": "2023-09-30"
                            }]
                        }
                    },
                    "AssetsCurrent": {
                        "units": {
                            "USD": [{
                                "val": 143566000000_u64,
                                "accn": "0000320193-23-000106",
                                "form": "10-K",
                                "end": "2023-09-30"
                            }]
                        }
                    },
                    "LiabilitiesCurrent": {
                        "units": {
                            "USD": [{
                                "val": 145308000000_u64,
                                "accn": "0000320193-23-000106",
                                "form": "10-K",
                                "end": "2023-09-30"
                            }]
                        }
                    },
                    "RelatedPartyTransactionsByRelatedParty": {
                        "units": {
                            "pure": [{
                                "val": "Supply agreement with related entity disclosed in note 12",
                                "accn": "0000320193-23-000106",
                                "form": "10-K",
                                "end": "2023-09-30"
                            }]
                        }
                    }
                }
            }
        })
    }

    fn inline_comp_fixture() -> serde_json::Value {
        serde_json::json!({
            "executive_compensation": [
                {
                    "officer_name": "Tim Cook",
                    "title": "Chief Executive Officer",
                    "total_compensation": 63209845,
                    "source_filing": "0000320193-24-000012"
                },
                {
                    "officer_name": "Luca Maestri",
                    "title": "Chief Financial Officer",
                    "total_compensation": 27000000,
                    "source_filing": "0000320193-24-000012"
                }
            ]
        })
    }

    #[test]
    fn sec_xbrl_parses_company_facts_and_extracts_revenue_across_periods() {
        let records = parse_company_facts(&company_facts_fixture());
        let annual = records.iter().find(|record| record.period_type == "annual").unwrap();
        let quarterly = records
            .iter()
            .find(|record| record.period_type == "quarterly")
            .unwrap();

        assert_eq!(records.len(), 2);
        assert_eq!(annual.cik, "0000320193");
        assert_eq!(annual.ticker.as_deref(), Some("AAPL"));
        assert_eq!(annual.financials.revenue, Some(394328000000.0));
        assert_eq!(quarterly.financials.revenue, Some(89500000000.0));
    }

    #[test]
    fn sec_xbrl_extracts_core_balance_sheet_and_income_statement_metrics() {
        let records = parse_company_facts(&company_facts_fixture());
        let annual = records.iter().find(|record| record.period_type == "annual").unwrap();

        assert_eq!(annual.financials.net_income, Some(96995000000.0));
        assert_eq!(annual.financials.total_assets, Some(352583000000.0));
        assert_eq!(annual.financials.total_liabilities, Some(290437000000.0));
        assert_eq!(annual.financials.stockholders_equity, Some(62146000000.0));
    }

    #[test]
    fn sec_xbrl_handles_annual_and_quarterly_periods() {
        let records = parse_company_facts(&company_facts_fixture());

        assert!(records.iter().any(|record| record.period_type == "annual"));
        assert!(records.iter().any(|record| record.period_type == "quarterly"));
        assert!(records.iter().any(|record| record.source_filing == "0000320193-24-000081"));
    }

    #[test]
    fn sec_xbrl_extracts_executive_compensation_from_inline_fixture() {
        let compensation = parse_inline_executive_compensation(&inline_comp_fixture());

        assert_eq!(compensation.len(), 2);
        assert_eq!(compensation[0].officer_name, "Tim Cook");
        assert_eq!(compensation[0].total_compensation, Some(63209845.0));
        assert_eq!(
            compensation[1].title.as_deref(),
            Some("Chief Financial Officer")
        );
    }

    #[test]
    fn sec_xbrl_calculates_current_ratio_debt_to_equity_and_profit_margin() {
        let records = parse_company_facts(&company_facts_fixture());
        let annual = records.iter().find(|record| record.period_type == "annual").unwrap();

        assert_eq!(annual.ratios.current_ratio.map(|v| v * 100.0).map(f64::round), Some(99.0));
        assert_eq!(annual.ratios.debt_to_equity.map(|v| (v * 100.0).round() / 100.0), Some(4.67));
        assert_eq!(annual.ratios.profit_margin.map(|v| (v * 1000.0).round() / 1000.0), Some(0.246));
    }

    #[test]
    fn sec_xbrl_handles_missing_metrics_gracefully() {
        let mut fixture = company_facts_fixture();
        fixture["facts"]["us-gaap"].as_object_mut().unwrap().remove("AssetsCurrent");
        fixture["facts"]["us-gaap"].as_object_mut().unwrap().remove("LiabilitiesCurrent");

        let records = parse_company_facts(&fixture);
        let annual = records.iter().find(|record| record.period_type == "annual").unwrap();

        assert_eq!(annual.financials.current_assets, None);
        assert_eq!(annual.financials.current_liabilities, None);
        assert_eq!(annual.ratios.current_ratio, None);
    }

    #[test]
    fn sec_xbrl_extracts_related_party_transactions() {
        let records = parse_company_facts(&company_facts_fixture());
        let annual = records.iter().find(|record| record.period_type == "annual").unwrap();

        assert_eq!(annual.related_party_transactions.len(), 1);
        assert_eq!(
            annual.related_party_transactions[0].concept,
            "RelatedPartyTransactionsByRelatedParty"
        );
        assert!(annual.related_party_transactions[0].value.contains("note 12"));
    }
}