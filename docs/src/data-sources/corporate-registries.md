# Corporate Registries

## GLEIF

Global LEI (Legal Entity Identifier) lookups via the [GLEIF API](https://www.gleif.org/en/lei-data/gleif-api).

```bash
redshank fetch gleif --lei "529900T8BM49AURSDO55"
redshank fetch gleif --name "Acme Corp"
```

## OpenCorporates

Company search and officer/subsidiary data via [OpenCorporates](https://api.opencorporates.com).

**Credential:** `OPENCORPORATES_API_KEY`

```bash
redshank fetch opencorporates --name "Acme Corp" --jurisdiction us_de
```

## FinCEN BOI

Beneficial Ownership Information filings from [FinCEN](https://boiefiling.fincen.gov).

```bash
redshank fetch fincen_boi --name "Acme Corp"
```

## State SOS Portals

Secretary of State business registry searches for Delaware, Florida, Nevada, and Wyoming. Pipelines configured in `redshank-fetchers/pipelines/state_sos/`.

```bash
redshank fetch state_sos --state DE --name "Acme Corp"
```

## SEC EDGAR

Company filings, ownership reports, and insider transactions via [EDGAR](https://efts.sec.gov/LATEST/search-index).

```bash
redshank fetch sec_edgar --name "Acme Corp"
redshank fetch sec_edgar --cik 0001234567 --form 13F
```

## FDIC

Bank and institution data from the [FDIC BankFind Suite](https://banks.data.fdic.gov/api).

```bash
redshank fetch fdic --name "First National Bank"
```

## PropPublica Nonprofit 990

IRS Form 990 filings via [PropPublica Nonprofit Explorer](https://projects.propublica.org/nonprofits/api).

```bash
redshank fetch propublica_990 --ein "12-3456789"
```
