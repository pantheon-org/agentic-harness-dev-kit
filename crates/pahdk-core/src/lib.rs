//! `pahdk-core`: shared configuration, error types, structured output, and
//! filesystem helpers for the Pantheon Agentic Harness Dev Kit tool crates
//! (pantheon-skill-auditor, pantheon-journal, pantheon-adr, pahdk-install).
//!
//! Populated in Wave 2 (A2). The public surface is intentionally small and
//! dependency-light so the tool crates can share it without pulling in each
//! other's concerns.

pub mod config;
pub mod error;
pub mod fs;
pub mod output;

pub use error::{Error, Result};
