# Security Policy

## Supported Versions

Redshank follows a latest-stable support model.

| Version | Supported |
| --- | --- |
| Latest release | Yes |
| Older releases | Best effort only |

Security fixes are generally shipped in the next patch release.

## Reporting a Vulnerability

Please do not open public issues for suspected vulnerabilities.

Use one of these private channels:

1. GitHub Security Advisory (preferred):
   [Security Advisory](https://github.com/greysquirr3l/redshank/security/advisories/new)
2. Email:
   [s0ma@protonmail.com](mailto:s0ma@protonmail.com)

Please include:

- Affected version and environment
- Reproduction steps or proof of concept
- Impact assessment (confidentiality, integrity, availability)
- Any suggested mitigation

## Response Timeline

- Initial acknowledgment: within 72 hours
- Triage decision: within 7 days
- Status updates: at least every 7 days while active

If the report is accepted, we will coordinate disclosure timing and credit unless you prefer to remain anonymous.

## Disclosure Guidelines

- Give maintainers reasonable time to investigate and patch before public disclosure.
- Avoid accessing, modifying, or exfiltrating data beyond what is necessary to demonstrate the issue.
- Do not run denial-of-service, destructive, or privacy-invasive tests against systems you do not own or have explicit permission to test.

## Scope Notes

Redshank integrates with third-party data providers and optional services.

- Vulnerabilities in upstream providers, external APIs, model providers, or third-party infrastructure should be reported to the relevant vendor.
- Vulnerabilities in Redshank code, release artifacts, workflow configuration, credential handling, or default local runtime behavior are in scope here.
