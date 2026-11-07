# Pantheon Agentic Harness Dev Kit (PAHDK)

Shared Rust libraries for the agent-harness tools in this organisation:
[pantheon-skill-auditor](https://github.com/pantheon-org/pantheon-skill-auditor),
[pantheon-journal](https://github.com/pantheon-org/pantheon-journal) and
[pantheon-adr](https://github.com/pantheon-org/pantheon-adr).

An agent harness is the tool an AI coding agent runs in, such as Claude Code or OpenCode. This is not
related to Harness.io or to the Holochain `hdk` crate.

| Crate | Purpose |
|---|---|
| `pahdk-core` | Shared configuration, error types, structured output and filesystem helpers; frontmatter splitting; and, behind the `validate` feature, validating a document against a JSON Schema and a heading list |
| `pahdk-install` | Agent-directory detection (42 agents) and bundled-skill installation |

The crates are at `0.0.0` and are not published yet. The API is internal to the tools above and may change
without notice until a `0.1.0` release.

## Validating documents

A tool that writes markdown with YAML frontmatter ships its shape as data: a `*.schema.json` for the
frontmatter and a list of required headings. With the `validate` feature, `pahdk-core` checks a document
against both and reports every problem with its location:

```toml
pahdk-core = { git = "https://github.com/pantheon-org/pantheon-agentic-harness-dev-kit.git", tag = "...", features = ["validate"] }
```

```rust
let issues = pahdk_core::validate::document_issues(&content, &schema, &required_headings)?;
```

`frontmatter::split` is available without the feature. Structure only: prose quality and whether a link
resolves stay with the calling tool.

## Installing a skill's git hook

A skill that needs a git hook ships a `hook.json` (`id`, `command`, optional `description`) next to its `SKILL.md`.
`pahdk_install::hooks` adds that hook to the configuration of the hook manager the repository already uses:

| Manager | File written | Notes |
|---|---|---|
| hk | `hk.pkl` | A `steps = name` line becomes `steps = (name) { ... }`; uninstall restores it |
| pre-commit | `.pre-commit-config.yaml` | A `repo: local` entry |
| lefthook | `lefthook.yml` (or `.yaml`, dotted) | A command under `pre-commit` |
| husky | `.husky/pre-commit` | A marked block of shell |

- `detect_manager` finds the manager. With none, or several, `install_hook` returns `NoManager` or `Ambiguous` and
  writes nothing; the calling CLI asks, then retries with `HookOptions::manager`.
- Everything the dev kit adds sits between `pahdk:begin <id>` and `pahdk:end <id>` comments. A second install finds
  the marker and changes nothing; `uninstall_hook` removes only the marked lines.
- `HookOptions::opt_out` writes nothing. `remember_opt_out` records the choice in `.git/pahdk-hooks.json` so an update
  does not ask again.
- The hook prints `[pahdk] <id>: running` to stderr, so the first commit shows it ran.
- A shape the dev kit cannot edit safely (for example `steps = makeSteps(1)` in `hk.pkl`) is an error that prints
  the snippet to add by hand.

## Development

```bash
cargo fmt --all --check
cargo clippy --workspace --all-targets --all-features --locked -- -D warnings
cargo test --workspace --all-features --locked
```

A pull request title is `type(scope): lowercase summary`, checked by the Conventional Commit Title workflow. Pull
requests are squash-merged, so the title is the commit message release-please reads. The allowed types are
release-please's defaults, listed in `.github/commit-types.txt`. The title workflow holds a copy of the list in its
regex, and `.github/scripts/check-commit-types.sh` fails if the two drift apart.

## History

Extracted from [pantheon-org/tekhne](https://github.com/pantheon-org/tekhne) with `git filter-repo`
(`crates/common` became `ahdk-core`, `crates/skill-install` became `ahdk-install`). Commit messages keep the
original `#NNN` references, which point at tekhne pull requests, not at this repository.

On 04-11-2026 the repository was renamed from `agentic-harness-dev-kit` to `pantheon-agentic-harness-dev-kit`, and the
crates from `ahdk-core` and `ahdk-install` to `pahdk-core` and `pahdk-install`, before the first crates.io publish.
Tag `v0.1.0-rc.1` still carries the old names.
