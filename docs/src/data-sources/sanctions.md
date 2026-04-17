# Sanctions

> Note: `redshank fetch` CLI dispatch currently exposes `uk_corporate_intelligence` only. The command snippets on this page document fetcher IDs and expected query shapes as dispatcher targets are expanded.

## OFAC SDN

Office of Foreign Assets Control Specially Designated Nationals list.

```bash
redshank fetch ofac_sdn --name "Ivan Petrov"
```

## UN Consolidated Sanctions

United Nations Security Council consolidated sanctions list.

```bash
redshank fetch un_sanctions --name "Ivan Petrov"
```

## EU Sanctions

European Union consolidated sanctions list.

```bash
redshank fetch eu_sanctions --name "Petrov"
```

## World Bank Debarred Firms

Firms and individuals debarred from World Bank Group projects.

```bash
redshank fetch world_bank_debarred --name "Acme Corp"
```
