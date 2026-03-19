# Data Sources Overview

Redshank ships 34 data fetchers across eight categories. Each fetcher implements the `DataFetcher` trait, respects rate limits, and writes NDJSON to stdout.

## Categories

| Category | Fetchers | Count |
|----------|----------|-------|
| [Campaign Finance](./campaign-finance.md) | FEC, Senate lobbying, House lobbying | 3 |
| [Government Contracts](./government-contracts.md) | USASpending, SAM.gov, FPDS, Federal Audit | 4 |
| [Corporate Registries](./corporate-registries.md) | GLEIF, OpenCorporates, FinCEN BOI, state SOS, SEC EDGAR | 5 |
| [Financial](./corporate-registries.md) | FDIC, PropPublica 990 filings | 2 |
| [Sanctions](./sanctions.md) | OFAC SDN, UN consolidated, EU sanctions, World Bank debarred | 4 |
| [Courts & Leaks](./courts-leaks.md) | CourtListener/RECAP, ICIJ offshore leaks | 2 |
| [Environmental & Reference](./environmental-reference.md) | EPA ECHO, OSHA, Census ACS, Wikidata, GDELT | 5 |
| [Individual OSINT](./individual-osint.md) | HIBP, GitHub, Wayback, WHOIS/RDAP, voter rolls, USPTO, username enum, social profiles | 8 |

## Running a fetcher directly

```bash
redshank fetch fec --name "ACME CORP" --type committee
redshank fetch gleif --lei "529900T8BM49AURSDO55"
redshank fetch ofac_sdn --name "Ivan Petrov"
```

Output is NDJSON — one JSON object per line — suitable for piping to `jq` or further processing.

## Rate limits and credentials

Each fetcher documents its rate limits and required credentials in its own page. All fetchers apply exponential backoff on `429` responses.
