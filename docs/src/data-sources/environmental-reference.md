# Environmental & Reference

## EPA ECHO

Compliance and enforcement data from the [EPA ECHO system](https://echo.epa.gov/tools/web-services).

```bash
redshank fetch epa_echo --name "Acme Manufacturing"
```

## OSHA Inspections

Workplace inspection records from the [OSHA API](https://enforcedata.dol.gov/views/data_api.php).

```bash
redshank fetch osha --establishment "Acme Corp"
```

## Census ACS

American Community Survey demographic data via the [Census API](https://api.census.gov).

```bash
redshank fetch census_acs --fips 12086 --variables B01001_001E,B19013_001E
```

## Wikidata

Entity facts and relationships via the [Wikidata SPARQL endpoint](https://query.wikidata.org).

```bash
redshank fetch wikidata --entity "Q312"
redshank fetch wikidata --sparql "SELECT ?item WHERE { ?item wdt:P31 wd:Q6256 }"
```

## GDELT

Global media event and tone analysis via the [GDELT 2.0 API](https://blog.gdeltproject.org/gdelt-2-0-our-global-world-in-realtime/).

```bash
redshank fetch gdelt --query "Acme Corp" --timespan 1month
```
