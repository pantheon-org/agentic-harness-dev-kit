# Security Policy

## Supported versions

This repository publishes library crates consumed by the other pantheon tools as git dependencies pinned by tag. Security fixes land on `main` and ship in the next tag; consumers should move to the latest tag.

| Version | Supported |
| ------- | --------- |
| Latest tag / `main` | Yes |
| Older tags | No |

## Reporting a vulnerability

Please do not open a public issue for security vulnerabilities.

Report privately through GitHub's built-in advisory workflow:

1. Go to the **Security** tab of this repository.
2. Select **Report a vulnerability** (Private vulnerability reporting).
3. Provide a clear description, affected paths or commands, reproduction steps,
   and any suggested remediation.

If private reporting is unavailable to you, contact the maintainers through the
repository's stated contact channel rather than a public issue.

## What to expect

- Acknowledgement of your report within 5 working days.
- An initial assessment and severity triage shortly after.
- Coordinated disclosure once a fix is available. We will credit reporters who
  wish to be named.

## Scope

In scope:

- The `pahdk-core` and `pahdk-install` crates under `crates/`, notably path
  handling and file writes when installing a skill into an agent's directory.
- CI/CD workflows under `.github/workflows/`.

Out of scope:

- Vulnerabilities in third-party dependencies (report those upstream; we track
  them through Dependabot).
- Findings that require a compromised developer machine or maintainer account.

## Handling of sensitive data

Do not include credentials, tokens, personal data, or other sensitive material
in a report. If a reproduction requires such data, use synthetic placeholders
and note that in your report.
