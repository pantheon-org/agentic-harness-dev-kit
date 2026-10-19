# Agentic Harness Dev Kit (AHDK)

Shared Rust libraries for the agent-harness tools in this organisation:
[pantheon-skill-auditor](https://github.com/pantheon-org/pantheon-skill-auditor),
[pantheon-journal](https://github.com/pantheon-org/pantheon-journal) and
[pantheon-adr](https://github.com/pantheon-org/pantheon-adr).

An agent harness is the tool an AI coding agent runs in, such as Claude Code or OpenCode. This is not
related to Harness.io or to the Holochain `hdk` crate.

| Crate | Purpose |
|---|---|
| `ahdk-core` | Shared configuration, error types, structured output and filesystem helpers |
| `ahdk-install` | Agent-directory detection (42 agents) and bundled-skill installation |

The crates are at `0.0.0` and are not published yet. The API is internal to the tools above and may change
without notice until a `0.1.0` release.

## Development

```bash
cargo fmt --all --check
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo test --workspace --locked
```

## History

Extracted from [pantheon-org/tekhne](https://github.com/pantheon-org/tekhne) with `git filter-repo`
(`crates/common` became `ahdk-core`, `crates/skill-install` became `ahdk-install`). Commit messages keep the
original `#NNN` references, which point at tekhne pull requests, not at this repository.
