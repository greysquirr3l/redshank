# Individual OSINT

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
