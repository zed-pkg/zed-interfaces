//! Core interface definitions for the zed-pkg universal package manager.
//!
//! This crate is the single source of truth for the on-disk formats
//! (`.zpkg.toml`, `.zpkg.lock`, store layout), the registry REST API DTOs, and
//! the publish-time exclusion rules. It is consumed by `zed-cli`,
//! `zed-api-server`, `zed-web-server`, and (via generated JSON Schemas in
//! `schemas/`) by the non-Rust client libraries in `zed-clients`.

pub mod artifact;
pub mod excludes;
pub mod inspection;
pub mod language;
pub mod lockfile;
pub mod manifest;
pub mod paths;
pub mod registry;
pub mod sync;
pub mod vcs;
pub mod version;

pub use artifact::ArtifactFormat;
pub use language::{Ecosystem, Language, detect_ecosystems};
pub use lockfile::{LockedPackage, Lockfile};
pub use manifest::{Manifest, ManifestError};
pub use vcs::Vcs;
pub use version::{Requirement, VersionScheme};
