# Campaign Finance

> Note: `redshank fetch` CLI dispatch currently exposes `uk_corporate_intelligence` only. The command snippets on this page document fetcher IDs and expected query shapes as dispatcher targets are expanded.

## FEC

Fetches campaign contributions, expenditures, and committee filings from the [FEC bulk data API](https://api.open.fec.gov).

**Credential:** `FEC_API_KEY`

```bash
redshank fetch fec --name "ACME CORP" --type committee
redshank fetch fec --candidate "John Smith" --cycle 2024
```

## Senate Lobbying Disclosures

Fetches lobbying registrations and activity reports from the [Senate LDA system](https://lda.senate.gov/api/).

```bash
redshank fetch senate_lobbying --registrant "Acme Lobbying LLC"
```

## House Lobbying Disclosures

Fetches House of Representatives lobbying disclosure forms from the [House Clerk system](https://disclosures.house.gov).

```bash
redshank fetch house_lobbying --registrant "Acme Lobbying LLC"
```
