# Contributing

Thanks for helping make independent event ephemerides accessible offline. **Read [AGENTS.md](AGENTS.md) before beginning**, including the no-publish/no-deploy/no-merge guardrails.

## Development workflow

- Look at [the roadmap](docs/ROADMAP.md), existing issues and draft PRs. Claim a small non-overlapping task. Prefer existing PRs for related changes.
- Use a topic branch; keep commits focused; tests and docs travel with code. No direct main commits for normal contributions, no GitHub Actions automation, no unapproved merge.
- Run the checks listed in [validation](docs/VALIDATION.md) when tooling exists; report both successful and unavailable checks precisely.
- Submit a **draft** PR. Fill the [PR template](.github/PULL_REQUEST_TEMPLATE.md) and update [work log](docs/WORK_LOG.md).
- Never ship or commit an actual database export or third-party dataset without owner approval and an approved dataset license.
- No credentials or personally identifying source records. If you find a vulnerability, follow [SECURITY.md](SECURITY.md) rather than posting exploit details.

## Code review principles

One canonical Rust engine, thin Python and JS/TS bindings, documented file-version compatibility, explicit time zone behavior, and tests comparing the same cases across languages. Keep an open extension path for new event families without breaking older files.

## Licensing

All contributed code is under [CPAL-1.0](LICENSE), subject to its attribution and source-distribution terms. Please preserve notices and record contributor copyright where relevant. Do not assume this grants rights to relicense another contributor's work later. The data are separately governed: see [Data provenance](docs/DATA_PROVENANCE.md).
