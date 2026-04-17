# Corporate Registries

> Note: `redshank fetch` CLI dispatch currently exposes `uk_corporate_intelligence` only. The command snippets on this page document fetcher IDs and expected query shapes as dispatcher targets are expanded.

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

### Licence — ODbL 1.0 (attribution required)

OpenCorporates data is published under the [Open Database Licence (ODbL 1.0)](https://opendatacommons.org/licenses/odbl/1-0/).
Every report, wiki entry, or UI element that surfaces OpenCorporates data **must** include a visible hyperlink:

> **[from OpenCorporates](https://opencorporates.com)** — or the canonical entity URL returned in the API response

Attribution rules from [opencorporates.com/terms-of-use-2](https://opencorporates.com/terms-of-use-2/):

- The link text must read **"from OpenCorporates"** and must resolve to the OpenCorporates homepage *or* the specific entity page (prefer the entity URL when available — it is returned in every API response).
- The link must be **at least 70 % the size of the largest font** used for the related information, and **never smaller than 7 px**, whichever of the two is larger.
- If OpenCorporates data forms the **substantial part** of a web page, add `<link rel="canonical" href="{entity_url}">` so search engines treat the OpenCorporates page as the authoritative source.
- If you expose this data through your own API, your downstream consumers inherit the same obligations.

#### How attribution propagates in redshank

`FetchOutput` for the `opencorporates` fetcher carries a populated `attribution` field:

```rust
pub struct Attribution {
    pub source: String,         // "OpenCorporates"
    pub text: String,           // "from OpenCorporates"
    pub url: String,            // "https://opencorporates.com"
    pub min_font_size_px: u8,   // 7
    pub licence: String,        // "ODbL-1.0"
}
```

The report and wiki-graph layers read `FetchOutput::attribution` and must render the hyperlink before writing any entity page that includes OpenCorporates fields. If the `url` field in a specific company record differs (i.e. the canonical entity URL was returned by the API), use that URL instead of the homepage.

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

## UK Corporate Intelligence

Merged UK company enrichment combining [Companies House](https://developer.company-information.service.gov.uk/) with [OpenCorporates](https://api.opencorporates.com).

**Credentials:** `UK_COMPANIES_HOUSE_API_KEY`, optional `OPENCORPORATES_API_KEY`

```bash
redshank fetch uk_corporate_intelligence --query "Acme Holdings"
```

When OpenCorporates contributes data, the same ODbL attribution rules above apply to downstream reports and UI.

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

## ProPublica Nonprofit 990

IRS Form 990 filings via [ProPublica Nonprofit Explorer](https://projects.propublica.org/nonprofits/api).

```bash
redshank fetch propublica_990 --ein "12-3456789"
```
