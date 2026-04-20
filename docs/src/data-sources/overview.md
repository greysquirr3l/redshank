# Data Sources Overview

Redshank ships 90+ fetcher modules across campaign finance, contracts, corporate registries, sanctions, courts, OSINT, environmental, and media categories.

The `redshank fetch` CLI dispatcher currently exposes UK corporate intelligence only; additional fetchers are available as crate modules and are being expanded into the dispatcher over time.

## Categories

| Category | Fetchers | Count |
|----------|----------|-------|
| [Campaign Finance](./campaign-finance.md) | FEC, Senate lobbying, House lobbying | 3 |
| [Government Contracts](./government-contracts.md) | USASpending, SAM.gov, FPDS, Federal Audit | 4 |
| [Corporate Registries](./corporate-registries.md) | GLEIF, OpenCorporates, FinCEN BOI, state SOS, SEC EDGAR | 5 |
| [Financial](./corporate-registries.md) | FDIC, ProPublica 990 filings | 2 |
| [Sanctions](./sanctions.md) | OFAC SDN, UN consolidated, EU sanctions, World Bank debarred | 4 |
| [Courts & Leaks](./courts-leaks.md) | CourtListener/RECAP, ICIJ offshore leaks | 2 |
| [Environmental & Reference](./environmental-reference.md) | EPA ECHO, OSHA, Census ACS, Wikidata, GDELT | 5 |
| [Individual OSINT](./individual-osint.md) | HIBP, GitHub, GitLab, Stack Exchange, Wayback, WHOIS/RDAP, voter rolls, USPTO, username enum, social profiles, reverse phone, reverse address | 12 |

## Running a fetcher directly

```bash
redshank fetch uk_corporate_intelligence --query "Acme Holdings" --output ./out
redshank fetch uk-corporate-intelligence --query "Acme Holdings" --output ./out
```

The CLI writes NDJSON files to the chosen output directory (one JSON object per line), suitable for ingestion with tools such as `jq`.

## Rate limits and credentials

Each fetcher documents its rate limits and required credentials in its own page. All fetchers apply exponential backoff on `429` responses.
