//! IRS Form 990 XML bulk parser and fetch helper.

use crate::domain::{FetchError, FetchOutput};
use crate::{build_client, write_ndjson};
use std::path::Path;

const INDEX_BASE: &str = "https://s3.amazonaws.com/irs-form-990";

/// Summary financial metrics extracted from Part I.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq)]
pub struct Irs990Summary {
    /// Total revenue for the filing period.
    pub total_revenue: Option<f64>,
    /// Total expenses for the filing period.
    pub total_expenses: Option<f64>,
    /// Revenue minus expenses.
    pub revenue_minus_expenses: Option<f64>,
    /// Total assets at year end.
    pub total_assets: Option<f64>,
    /// Total liabilities at year end.
    pub total_liabilities: Option<f64>,
    /// Net assets or fund balances.
    pub net_assets: Option<f64>,
    /// Governing body size.
    pub voting_members: Option<u32>,
    /// Total employees.
    pub employees: Option<u32>,
    /// Total volunteers.
    pub volunteers: Option<u32>,
}

/// Compensation record for an officer or key employee.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq)]
pub struct CompensationRecord {
    /// Individual name.
    pub name: String,
    /// Title or role.
    pub title: Option<String>,
    /// Base reportable compensation.
    pub reportable_compensation: Option<f64>,
    /// Other compensation or benefits.
    pub other_compensation: Option<f64>,
}

/// Schedule A public support details.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq)]
pub struct ScheduleASupport {
    /// Public charity classification text.
    pub charity_type: Option<String>,
    /// Public support numerator or aggregate support value.
    pub public_support: Option<f64>,
    /// Total support denominator.
    pub total_support: Option<f64>,
    /// Public support percentage if present.
    pub support_percentage: Option<f64>,
}

/// Schedule B contributors summary.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq)]
pub struct ScheduleBContributors {
    /// Number of contributors in the public filing.
    pub contributor_count: Option<u32>,
    /// Aggregate contributions reported.
    pub total_contributions: Option<f64>,
}

/// Schedule R related organization summary.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
pub struct RelatedOrganization {
    /// Related organization legal name.
    pub name: String,
    /// Relationship description.
    pub relationship: Option<String>,
}

/// Normalized IRS 990 XML filing.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq)]
pub struct Irs990XmlRecord {
    /// Filing EIN.
    pub ein: String,
    /// Tax year or tax period.
    pub tax_period: Option<String>,
    /// IRS form type.
    pub form_type: Option<String>,
    /// Organization name.
    pub organization_name: Option<String>,
    /// Summary financial metrics.
    pub summary: Irs990Summary,
    /// Top compensation records.
    pub top_compensation: Vec<CompensationRecord>,
    /// Schedule A public support data.
    pub schedule_a: Option<ScheduleASupport>,
    /// Schedule B contributor totals.
    pub schedule_b: Option<ScheduleBContributors>,
    /// Supplemental narrative text from Schedule O.
    pub schedule_o_text: Vec<String>,
    /// Related organizations from Schedule R.
    pub related_organizations: Vec<RelatedOrganization>,
}

fn extract_tag(text: &str, tag: &str) -> Option<String> {
    let open = format!("<{tag}>");
    let close = format!("</{tag}>");
    let start = text.find(&open)? + open.len();
    let remainder = text.get(start..)?;
    let end = remainder.find(&close)?;
    Some(remainder[..end].trim().to_string())
}

fn extract_first_tag(text: &str, tags: &[&str]) -> Option<String> {
    tags.iter().find_map(|tag| extract_tag(text, tag))
}

fn parse_amount(text: &str, tags: &[&str]) -> Option<f64> {
    extract_first_tag(text, tags).and_then(|value| value.replace(',', "").parse::<f64>().ok())
}

fn parse_u32(text: &str, tags: &[&str]) -> Option<u32> {
    extract_first_tag(text, tags).and_then(|value| value.replace(',', "").parse::<u32>().ok())
}

fn collect_blocks<'a>(text: &'a str, tag: &str) -> Vec<&'a str> {
    let open = format!("<{tag}>");
    let close = format!("</{tag}>");
    let mut blocks = Vec::new();
    let mut remainder = text;

    while let Some(start) = remainder.find(&open) {
        let after_open = &remainder[start + open.len()..];
        let Some(end) = after_open.find(&close) else {
            break;
        };
        blocks.push(&after_open[..end]);
        remainder = &after_open[end + close.len()..];
    }

    blocks
}

fn collect_schedule_o_text(text: &str) -> Vec<String> {
    collect_blocks(text, "IRS990ScheduleO")
        .into_iter()
        .flat_map(|block| {
            collect_blocks(block, "ExplanationTxt")
                .into_iter()
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(ToString::to_string)
                .collect::<Vec<_>>()
        })
        .collect()
}

fn collect_related_organizations(text: &str) -> Vec<RelatedOrganization> {
    collect_blocks(text, "RelatedOrganizationDetail")
        .into_iter()
        .filter_map(|block| {
            let name = extract_first_tag(block, &["BusinessNameLine1Txt", "NameBusinessNameLine1Txt"])?;
            Some(RelatedOrganization {
                name,
                relationship: extract_first_tag(
                    block,
                    &["PrimaryActivityTxt", "RelationshipDescriptionTxt"],
                ),
            })
        })
        .collect()
}

fn collect_compensation(text: &str) -> Vec<CompensationRecord> {
    collect_blocks(text, "Form990PartVIISectionAGrp")
        .into_iter()
        .filter_map(|block| {
            let name = extract_first_tag(block, &["PersonNm"])?;
            Some(CompensationRecord {
                name,
                title: extract_first_tag(block, &["TitleTxt"]),
                reportable_compensation: parse_amount(block, &["ReportableCompFromOrgAmt"]),
                other_compensation: parse_amount(
                    block,
                    &[
                        "OtherCompensationAmt",
                        "EstimatedAmtOfOtherCompAmt",
                        "OtherCompensationFromOrgAmt",
                    ],
                ),
            })
        })
        .take(5)
        .collect()
}

/// Parse a single IRS 990 XML filing.
#[must_use]
pub fn parse_irs_990_xml(ein: &str, xml: &str) -> Option<Irs990XmlRecord> {
    Some(Irs990XmlRecord {
        ein: ein.to_string(),
        tax_period: extract_first_tag(xml, &["TaxPeriodEndDt", "TaxYr"]),
        form_type: extract_first_tag(xml, &["ReturnTypeCd", "ReturnType", "FormTypeCd"]),
        organization_name: extract_first_tag(xml, &["BusinessNameLine1Txt"]),
        summary: Irs990Summary {
            total_revenue: parse_amount(xml, &["CYTotalRevenueAmt", "TotalRevenueCurrentYearAmt"]),
            total_expenses: parse_amount(xml, &["CYTotalExpensesAmt", "TotalExpensesCurrentYearAmt"]),
            revenue_minus_expenses: parse_amount(
                xml,
                &["RevenueLessExpensesAmt", "CYRevenuesLessExpensesAmt"],
            ),
            total_assets: parse_amount(xml, &["TotalAssetsEOYAmt"]),
            total_liabilities: parse_amount(xml, &["TotalLiabilitiesEOYAmt"]),
            net_assets: parse_amount(xml, &["NetAssetsOrFundBalancesEOYAmt"]),
            voting_members: parse_u32(xml, &["VotingMembersGoverningBodyCnt"]),
            employees: parse_u32(xml, &["TotalEmployeeCnt"]),
            volunteers: parse_u32(xml, &["TotalVolunteersCnt"]),
        },
        top_compensation: collect_compensation(xml),
        schedule_a: Some(ScheduleASupport {
            charity_type: extract_first_tag(
                xml,
                &[
                    "PublicCharityStatusTxt",
                    "Organization509a1TypeInd",
                    "Organization509a2TypeInd",
                ],
            ),
            public_support: parse_amount(xml, &["PublicSupportAmt"]),
            total_support: parse_amount(xml, &["TotalSupportAmt"]),
            support_percentage: parse_amount(xml, &["PublicSupportPercentageTxt"]),
        })
        .filter(|value| {
            value.charity_type.is_some()
                || value.public_support.is_some()
                || value.total_support.is_some()
                || value.support_percentage.is_some()
        }),
        schedule_b: Some(ScheduleBContributors {
            contributor_count: parse_u32(xml, &["ContributorCnt"]),
            total_contributions: parse_amount(xml, &["TotalContributionsAmt"]),
        })
        .filter(|value| value.contributor_count.is_some() || value.total_contributions.is_some()),
        schedule_o_text: collect_schedule_o_text(xml),
        related_organizations: collect_related_organizations(xml),
    })
}

/// Fetch an IRS 990 XML record for an EIN and year using the public index.
///
/// # Errors
///
/// Returns `Err` if the index or filing fetch fails or parsing fails.
pub async fn fetch_irs_990_xml(
    ein: &str,
    year: u32,
    output_dir: &Path,
) -> Result<FetchOutput, FetchError> {
    let client = build_client()?;
    let index_url = format!("{INDEX_BASE}/index_{year}.json");
    let index_resp = client.get(&index_url).send().await?;
    let index_status = index_resp.status();
    if !index_status.is_success() {
        let body = index_resp.text().await.unwrap_or_default();
        return Err(FetchError::ApiError {
            status: index_status.as_u16(),
            body,
        });
    }

    let index_json: serde_json::Value = index_resp.json().await?;
    let entry = index_json
        .as_array()
        .and_then(|entries| {
            entries.iter().find(|entry| {
                entry
                    .get("EIN")
                    .or_else(|| entry.get("ein"))
                    .and_then(serde_json::Value::as_str)
                    == Some(ein)
            })
        })
        .ok_or_else(|| FetchError::Parse(format!("no IRS 990 index entry found for EIN {ein}")))?;

    let filing_url = if let Some(url) = entry
        .get("URL")
        .or_else(|| entry.get("Url"))
        .and_then(serde_json::Value::as_str)
    {
        url.to_string()
    } else if let Some(object_id) = entry.get("ObjectId").and_then(serde_json::Value::as_str) {
        format!("{INDEX_BASE}/{object_id}")
    } else {
        return Err(FetchError::Parse(
            "IRS 990 index entry missing filing URL".to_string(),
        ));
    };

    let filing_resp = client.get(&filing_url).send().await?;
    let filing_status = filing_resp.status();
    if !filing_status.is_success() {
        let body = filing_resp.text().await.unwrap_or_default();
        return Err(FetchError::ApiError {
            status: filing_status.as_u16(),
            body,
        });
    }

    let xml = filing_resp.text().await?;
    let record = parse_irs_990_xml(ein, &xml)
        .ok_or_else(|| FetchError::Parse("could not parse IRS 990 XML filing".to_string()))?;
    let output_path = output_dir.join("irs_990_xml.ndjson");
    let records =
        vec![serde_json::to_value(record).map_err(|err| FetchError::Parse(err.to_string()))?];
    let count = write_ndjson(&output_path, &records)?;

    Ok(FetchOutput {
        records_written: count,
        output_path,
        source_name: "irs-990-xml".into(),
        attribution: None,
    })
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::indexing_slicing, clippy::panic)]
mod tests {
    use super::*;

    fn filing_fixture() -> &'static str {
        r"
        <Return>
          <TaxPeriodEndDt>2024-12-31</TaxPeriodEndDt>
          <ReturnTypeCd>990</ReturnTypeCd>
          <BusinessNameLine1Txt>Investigative Journalism Fund</BusinessNameLine1Txt>
          <CYTotalRevenueAmt>1250000</CYTotalRevenueAmt>
          <CYTotalExpensesAmt>910000</CYTotalExpensesAmt>
          <RevenueLessExpensesAmt>340000</RevenueLessExpensesAmt>
          <TotalAssetsEOYAmt>2200000</TotalAssetsEOYAmt>
          <TotalLiabilitiesEOYAmt>180000</TotalLiabilitiesEOYAmt>
          <NetAssetsOrFundBalancesEOYAmt>2020000</NetAssetsOrFundBalancesEOYAmt>
          <VotingMembersGoverningBodyCnt>9</VotingMembersGoverningBodyCnt>
          <TotalEmployeeCnt>17</TotalEmployeeCnt>
          <TotalVolunteersCnt>44</TotalVolunteersCnt>

          <IRS990ScheduleA>
            <PublicCharityStatusTxt>509(a)(1)</PublicCharityStatusTxt>
            <PublicSupportAmt>980000</PublicSupportAmt>
            <TotalSupportAmt>1100000</TotalSupportAmt>
            <PublicSupportPercentageTxt>89.1</PublicSupportPercentageTxt>
          </IRS990ScheduleA>

          <IRS990ScheduleB>
            <ContributorCnt>27</ContributorCnt>
            <TotalContributionsAmt>765000</TotalContributionsAmt>
          </IRS990ScheduleB>

          <Form990PartVIISectionAGrp>
            <PersonNm>Alex Carter</PersonNm>
            <TitleTxt>Executive Director</TitleTxt>
            <ReportableCompFromOrgAmt>210000</ReportableCompFromOrgAmt>
            <OtherCompensationAmt>18000</OtherCompensationAmt>
          </Form990PartVIISectionAGrp>
          <Form990PartVIISectionAGrp>
            <PersonNm>Priya Shah</PersonNm>
            <TitleTxt>Chief Operating Officer</TitleTxt>
            <ReportableCompFromOrgAmt>175000</ReportableCompFromOrgAmt>
            <OtherCompensationAmt>14000</OtherCompensationAmt>
          </Form990PartVIISectionAGrp>

          <IRS990ScheduleO>
            <ExplanationTxt>Expanded investigative training programs across three states.</ExplanationTxt>
            <ExplanationTxt>Documented conflict-of-interest review procedures for board members.</ExplanationTxt>
          </IRS990ScheduleO>

          <RelatedOrganizationDetail>
            <BusinessNameLine1Txt>IJF Action Fund</BusinessNameLine1Txt>
            <RelationshipDescriptionTxt>Related 501(c)(4) advocacy affiliate</RelationshipDescriptionTxt>
          </RelatedOrganizationDetail>
        </Return>
        "
    }

    #[test]
    fn irs_990_xml_parses_summary_financials() {
        let record = parse_irs_990_xml("123456789", filing_fixture()).unwrap();

        assert_eq!(record.summary.total_revenue, Some(1_250_000.0));
        assert_eq!(record.summary.total_expenses, Some(910_000.0));
        assert_eq!(record.summary.net_assets, Some(2_020_000.0));
        assert_eq!(record.summary.employees, Some(17));
    }

    #[test]
    fn irs_990_xml_extracts_schedule_a_and_b_details() {
        let record = parse_irs_990_xml("123456789", filing_fixture()).unwrap();

        let schedule_a = record.schedule_a.unwrap();
        assert_eq!(schedule_a.charity_type.as_deref(), Some("509(a)(1)"));
        assert_eq!(schedule_a.public_support, Some(980_000.0));
        assert_eq!(schedule_a.support_percentage, Some(89.1));

        let schedule_b = record.schedule_b.unwrap();
        assert_eq!(schedule_b.contributor_count, Some(27));
        assert_eq!(schedule_b.total_contributions, Some(765_000.0));
    }

    #[test]
    fn irs_990_xml_extracts_compensation_and_schedule_o_text() {
        let record = parse_irs_990_xml("123456789", filing_fixture()).unwrap();

        assert_eq!(record.top_compensation.len(), 2);
        assert_eq!(record.top_compensation[0].name, "Alex Carter");
        assert_eq!(
            record.top_compensation[1].title.as_deref(),
            Some("Chief Operating Officer")
        );
        assert_eq!(record.schedule_o_text.len(), 2);
        assert!(record.schedule_o_text[0].contains("training programs"));
    }
}