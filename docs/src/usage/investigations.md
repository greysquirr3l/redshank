# Running Investigations

## Writing a good objective

The objective should be specific enough for the agent to know when it's done, but open enough to allow autonomous tool selection.

**Too vague:**

```
Tell me about this company.
```

**Too prescriptive:**

```
Look up Acme Corp in GLEIF, then OpenCorporates, then FinCEN BOI, then cross-reference with OFAC.
```

**Just right:**

```
Identify all beneficial owners of Acme Corp and any related entities that appear on OFAC, UN, or EU sanctions lists.
```

## Fetcher chaining

The agent decides which fetchers to call based on the objective. Common patterns:

**Corporate investigation:**
GLEIF (LEI) → OpenCorporates (subsidiaries) → FinCEN BOI (beneficial owners) → OFAC/UN/EU (sanctions check) → SEC EDGAR (filings) → state SOS portals

**Political finance:**
FEC (contributions) → Senate/House lobbying disclosures → USASpending (contracts) → SAM.gov (registrations) → FPDS (awards)

**Individual OSINT:**
HIBP (breach exposure) → GitHub profile → WHOIS/RDAP → Wayback Machine → voter rolls → USPTO inventors

## Multi-depth investigations

Use `--max-depth` to allow the agent to spin up child agents for subtasks:

```bash
redshank run --max-depth 3 "Map the full ownership network behind the top 10 SAM.gov contractors in the defense sector"
```

Each child invocation gets its own step budget and writes its findings back to the shared wiki.

## Reviewing results

All findings are written to `wiki/` in the working directory:

- `wiki/index.md` — master index of all entities discovered
- `wiki/<entity-slug>.md` — per-entity pages with sourced claims and cross-references

The wiki is plain Markdown and can be committed to a repository, rendered with any static site generator, or read directly.
