# T21 — 8 individual-person OSINT fetchers (HIBP, GitHub, Wayback, WHOIS/RDAP, voter rolls, USPTO, username enum, social profiles)

> **Depends on**: T-fetchers-extended, T-stygian-integration.

## Goal

Add 8 individual-person OSINT fetchers that fill a gap entirely absent from
OpenPlanter. These cover breach-exposure checking, public username/identity
correlation, historical web presence, domain registration history, voter
registration records, patent/trademark inventor data, and JS-rendered social
profile scraping. All sources are fully public or breach-notification services
that return only exposure metadata — never raw credential material.


## Project Context

- Project: `redshank` — Redshank is an autonomous recursive language-model investigation agent written
in Rust 1.94 (edition 2024). It ingests heterogeneous public datasets — campaign
finance, lobbying disclosures, federal contracts, corporate registries,
sanctions lists (OFAC, UN, EU, World Bank), property records, nonprofit
filings, corporate registries (GLEIF, OpenCorporates, FinCEN BOI, state SOS
portals), federal courts (RECAP/CourtListener), individual-person OSINT
(breach exposure, username enumeration across 300+ platforms, voter rolls,
github profiles, WHOIS history, patent/trademark inventors), and media
intelligence (GDELT) — resolves entities across all of them, and surfaces
non-obvious connections through evidence-backed analysis written into a live
knowledge-graph wiki.

The agent runs a tool-calling loop that can recursively delegate subtasks to
child agent invocations, condense context on long runs, apply a cheap judge
model to evaluate acceptance criteria, and stream its reasoning to an interactive
ratatui TUI. Web fetches use stygian-graph pipelines (with optional stygian-browser
anti-detection automation for JS-rendered pages). A compiled binary ships as a
single executable with no Python or Node.js runtime dependency.

- Language: rust
- Architecture: hexagonal-ddd-cqrs-security-first



## Strategy: TDD (Red-Green-Refactor)

### 1. RED — Write failing tests first

- Test: HIBP fetcher constructs correct URL, sets hibp-api-key header, parses breach list fixture JSON, and NEVER logs the API key.
- Test: HIBP fetcher returns empty list (not an error) when API returns 404 (clean email).
- Test: GitHub profile fetcher parses user fixture and returns org membership list.
- Test: GitHub email-reverse-lookup constructs correct search query URL.
- Test: Wayback CDX fetcher parses array-of-arrays JSON response into typed snapshot records.
- Test: RDAP fetcher parses domain fixture JSON and extracts registrar + creation date.
- Test: USPTO PatentsView fetcher constructs inventor name query and parses patent list fixture.
- Test: username-enum pipeline config loads platforms.toml and constructs correct URL for GitHub template.
- Test: username-enum marks platform as found=true on 200, found=false on 404.
- Test (stygian feature): social-profiles pipeline config validates without launching a browser.
- Test: voter-reg fetcher parses NC tab-delimited fixture row into structured output.


### 2. GREEN — Implement to pass

- fetch-hibp: Have I Been Pwned API v3. GET https://haveibeenpwned.com/api/v3/breachedaccount/{email}?truncateResponse=false with hibp-api-key header. Returns list of breach names, domains, dates, and data classes exposed (email, password-hash, phone, etc.). IMPORTANT: this API returns only breach METADATA — it never returns actual passwords or raw breach data. Use to answer 'was an email address found in a known breach?' during an investigation. Free for individual lookups at 1.5 req/sec. Add hibp_api_key to CredentialBundle.
- fetch-github-profile: GitHub REST API. GET https://api.github.com/users/{username} and /users/{username}/orgs and /repos. No auth for public data (60 req/hr); add github_token to CredentialBundle for 5000 req/hr. Output: name, bio, company, location, email (if public), org memberships, public repo list. Surfaces tech workers, security researchers, and developers who use their real names in commits. Also: GET https://api.github.com/search/users?q={email}+in:email for email→username reverse lookup.
- fetch-wayback: Wayback Machine CDX Server API. GET https://web.archive.org/cdx/search/cdx?url={domain_or_url}&output=json&limit=500. Returns (urlkey, timestamp, original, mimetype, statuscode, digest, length) for each archived snapshot. Use to reconstruct a domain's historical content and detect when it changed ownership. No auth, no rate limit stated — be polite (500ms between requests).
- fetch-whois-rdap: RDAP (Registration Data Access Protocol) — the WHOIS successor. GET https://rdap.org/domain/{domain} or https://rdap.verisign.com/com/v1/domain/{domain}. Returns registrant org (often privacy-masked), creation/expiry dates, nameservers, registrar. For IP blocks: GET https://rdap.arin.net/registry/ip/{ip}. No auth. Pair with Wayback results to build a domain provenance timeline.
- fetch-voter-reg: State voter registration bulk files. Free/cheap states: FL (https://dos.fl.gov/elections/data-statistics/voter-registration-statistics/voter-extract-disk-request/), NC (https://www.ncsbe.gov/results-data/voter-registration-data), OH (https://www.ohiosos.gov/elections/voters/find-my-voter-registration/). Each provides a tab-delimited or CSV file (zip download). Fields include: full name, address, party, registration date, voting history. Use stygian-browser for portal navigation where direct download URL is not static. Name + address from voter rolls is a powerful cross-link to property records and campaign donations.
- fetch-uspto: USPTO PatentsView API and TMAPI. GET https://search.patentsview.org/api/v1/inventor/?q={"inventor_last_name":"Smith"}  and https://developer.uspto.gov/api-catalog/trademark-search. Patent inventors and trademark applicants/assignees are filed under real individual names and addresses — surfaces undisclosed company interests and technical expertise of persons under investigation. No auth for PatentsView (1000 req/day free); TMAPI uses api.gov key.
- fetch-username-enum: stygian-graph pipeline. Given a username, construct profile URLs across 300+ platforms (GitHub, Reddit, HackerNews, Twitter/X, Mastodon, Instagram, TikTok, YouTube, GitLab, npm, PyPI, Docker Hub, Steam, Keybase, Medium, Substack, etc.) and HEAD-request each to detect 200 vs 404. Compile presence map. Store platform URL templates as TOML in src/pipelines/username_enum/platforms.toml — one entry per platform with url_template, success_codes, and false_positive_patterns. Use stygian-graph http nodes with parallel wave execution for throughput. Output NDJSON: {username, platform, url, found: bool}. NO password/credential extraction of any kind.
- fetch-social-profiles: stygian-graph pipeline with stygian-browser Advanced stealth + ai_extract node. Target platforms: LinkedIn public profiles (https://www.linkedin.com/in/{slug}), Twitter/X (https://x.com/{username}), Mastodon (ActivityPub public JSON: https://{instance}/@{username}/outbox). Pipeline: browser navigate → wait for network idle → ai_extract node (Claude) extracts: display name, headline/bio, employer, location, follower count, recent post summary, linked URLs. Output NDJSON. LinkedIn ToS note: only access public profiles that are indexable by search engines (robots.txt allows). Pipeline config in src/pipelines/social_profiles/. Rate: one profile per 5 seconds minimum.


### 3. REFACTOR — Clean up while green

- Remove duplication
- Improve naming and structure
- Keep all tests passing


## Housekeeping: TODO / FIXME Sweep

Before running preflight, scan all files you created or modified in this task for
`TODO`, `FIXME`, `HACK`, `XXX`, and similar markers.

- **Resolve** any that fall within the scope of this task's goal.
- **Leave in place** any that reference work belonging to a later task or phase — but ensure they include a task reference (e.g. `// TODO(T07): wire up auth adapter`).
- **Remove** any placeholder markers that are no longer relevant after your implementation.

If none are found, move on.

## Preflight

```bash
cargo build --workspace && cargo test --workspace && cargo clippy --workspace -- -D warnings
```

## Exit Criteria

- [ ] All code compiles without errors or warnings
- [ ] All tests pass
- [ ] Linter passes with no warnings
- [ ] Implementation matches the goal described above
- [ ] No unresolved TODO/FIXME/HACK markers that belong to this task's scope

## After Completion

Update PROGRESS.md row for T21 to `[x]`.
Commit: `feat(fetchers-osint): implement 8 individual-person osint fetchers (hibp, github, wayback, whois/rdap, voter rolls, uspto, username enum, social profiles)`
