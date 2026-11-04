# Pantheon Agentic Harness Dev Kit (PAHDK)

Shared Rust libraries for the agent-harness tools in this organisation:
[pantheon-skill-auditor](https://github.com/pantheon-org/pantheon-skill-auditor),
[pantheon-journal](https://github.com/pantheon-org/pantheon-journal) and
[pantheon-adr](https://github.com/pantheon-org/pantheon-adr).

An agent harness is the tool an AI coding agent runs in, such as Claude Code or OpenCode. This is not
related to Harness.io or to the Holochain `hdk` crate.

| Crate | Purpose |
|---|---|
| `pahdk-core` | Shared configuration, error types, structured output and filesystem helpers |
| `pahdk-install` | Agent-directory detection (42 agents) and bundled-skill installation |

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

On 04-11-2026 the repository was renamed from `agentic-harness-dev-kit` to `pantheon-agentic-harness-dev-kit`, and the
crates from `ahdk-core` and `ahdk-install` to `pahdk-core` and `pahdk-install`, before the first crates.io publish.
Tag `v0.1.0-rc.1` still carries the old names.
