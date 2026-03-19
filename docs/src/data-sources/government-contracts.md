# Government Contracts

## USASpending

Federal contract and grant awards from [USASpending.gov](https://api.usaspending.gov).

```bash
redshank fetch usaspending --recipient "Acme Corp" --award-type contract
```

## SAM.gov

Entity registrations and exclusions from [SAM.gov](https://api.sam.gov).

```bash
redshank fetch sam_gov --name "Acme Corp"
```

## FPDS

Contract awards from the Federal Procurement Data System.

```bash
redshank fetch fpds --vendor "Acme Corp" --naics 541511
```

## Federal Audit Clearinghouse

Single audit findings from the [Federal Audit Clearinghouse](https://facdissem.census.gov).

```bash
redshank fetch federal_audit --ein "12-3456789"
```
