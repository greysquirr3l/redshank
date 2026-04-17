# Courts & Leaks

> Note: `redshank fetch` CLI dispatch currently exposes `uk_corporate_intelligence` only. The command snippets on this page document fetcher IDs and expected query shapes as dispatcher targets are expanded.

## CourtListener (RECAP)

Federal court dockets and documents via the [CourtListener RECAP API](https://www.courtlistener.com/api/).

```bash
redshank fetch courtlistener --party "Acme Corp"
redshank fetch courtlistener --docket 1234567
```

## ICIJ Offshore Leaks

Entities from the Panama Papers, Pandora Papers, and other ICIJ leak databases via the [ICIJ Offshore Leaks API](https://offshoreleaks.icij.org/api).

```bash
redshank fetch icij_leaks --name "Acme Corp"
```
