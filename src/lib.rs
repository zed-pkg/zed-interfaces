//! Core interface definitions for the zed-pkg universal package manager.
//!
//! This crate is the single source of truth for the on-disk formats
//! (`zed.toml`, `zed.lock`, store layout), the registry REST API DTOs, and
//! the publish-time exclusion rules. It is consumed by `zed-cli`,
//! `zed-api-server`, `zed-web-server`, and (via generated JSON Schemas in
//! `schemas/`) by the non-Rust client libraries in `zed-clients`.

pub mod artifact;
pub mod excludes;
pub mod lockfile;
pub mod manifest;
pub mod paths;
pub mod registry;
pub mod vcs;

pub use artifact::ArtifactFormat;
pub use lockfile::{LockedPackage, Lockfile};
pub use manifest::{Manifest, ManifestError};
pub use vcs::Vcs;
