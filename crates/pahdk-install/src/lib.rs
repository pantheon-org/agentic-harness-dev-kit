//! `pahdk-install`: agent-directory detection and bundled-skill installation
//! for the Pantheon Agentic Harness Dev Kit tool CLIs.
//!
//! Ported from `cli/lib/types/agents.ts` and `cli/lib/install`. The crate
//! exposes three concerns, one per module:
//!
//! - [`env`]: base-directory resolution (`home`, `configHome`, `codexHome`,
//!   `claudeHome`) including environment overrides and XDG semantics.
//! - [`agents`]: the 42-agent table, each with its skills directory, global
//!   directory resolution, and `detectInstalled` probe paths.
//! - [`install`]: resolving an agent's target directory and copying or
//!   symlinking a bundled skill into it.
//! - [`hooks`]: installing and removing the git hook a skill declares, in the
//!   configuration of the repository's own hook manager.

pub mod agents;
pub mod env;
pub mod hooks;
pub mod install;

pub use agents::{all, by_name, slugs, AgentConfig, Base, DetectPath, GlobalDir, AGENTS};
pub use env::{
    open_claw_global_skills_dir, resolve_config_home, resolve_home_override, Environment,
};
pub use hooks::{
    detect_manager, install_hook, is_opted_out, remember_opt_out, uninstall_hook, Detection,
    HookOptions, HookOutcome, HookSpec, Manager,
};
pub use install::{install_skill, resolve_target_dir, InstallMode};
