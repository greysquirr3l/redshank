# Individual OSINT

> Note: `redshank fetch` CLI dispatch currently exposes `uk_corporate_intelligence` only. The command snippets on this page document fetcher IDs and expected query shapes as dispatcher targets are expanded.

## HIBP — Have I Been Pwned

Breach exposure lookup via the [HIBP API](https://haveibeenpwned.com/API/v3).

**Credential:** `HIBP_API_KEY`

```bash
redshank fetch hibp --email "user@example.com"
```

## GitHub Profiles

Public profile, repositories, and contribution history via the [GitHub API](https://docs.github.com/en/rest).

```bash
redshank fetch github_profile --username "octocat"
```

## GitLab Profiles

Public GitLab profile search via the [GitLab Users API](https://docs.gitlab.com/ee/api/users.html).

```bash
redshank fetch gitlab_profile --query "jane investigator"
```

## Stack Exchange Profiles

Public Stack Overflow/Stack Exchange profile lookup via the [Stack Exchange API](https://api.stackexchange.com/).

```bash
redshank fetch stackexchange_profile --query "Jane Investigator"
```

## Wayback Machine

Historical snapshots of a domain or URL via the [Wayback CDX API](https://web.archive.org/cdx/search/cdx).

```bash
redshank fetch wayback --url "example.com"
```

## WHOIS / RDAP

Domain registration and RDAP lookups.

```bash
redshank fetch whois_rdap --domain "example.com"
```

## Voter Registration

Voter roll data from state portals. Currently supports North Carolina.

```bash
redshank fetch voter_reg --state NC --last-name "Smith" --first-name "John"
```

## USPTO Patent & Trademark

Patent and trademark filings with inventor/applicant search via the [USPTO API](https://developer.uspto.gov).

```bash
redshank fetch uspto --inventor-last "Smith" --inventor-first "John"
```

## Username Enumeration

Check username availability across 37+ platforms. Platforms configured in `redshank-fetchers/pipelines/username_enum/platforms.toml`.

```bash
redshank fetch username_enum --username "jsmith"
```

## Social Profiles

Social media profile enumeration across configured platforms. Pipelines in `redshank-fetchers/pipelines/social_profiles/`.

```bash
redshank fetch social_profiles --username "jsmith"
```

## Reverse Phone (Basic)

Best-effort phone normalization and country-code metadata hints (no paid identity lookup).

```bash
redshank fetch reverse_phone_basic --query "+1 415 555 2671"
```

## Reverse Address (Public)

Public address normalization/geocoding via the U.S. Census geocoder.

```bash
redshank fetch reverse_address_public --query "1600 Pennsylvania Ave NW, Washington, DC"
```
