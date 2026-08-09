//! Filesystem layout conventions shared by the CLI, servers, and docs.
//!
//! The global store lives under `$HOME/.zed-pkg` (overridable with
//! `ZED_PKG_HOME`). Projects never copy packages by default: installed
//! versions are extracted once into the content-addressed store and
//! symlinked into the project's `zed_modules/` directory, pnpm-style.
//! In containers (`--install-mode copy`) the symlink step is replaced by a
//! copy so image layers stay self-contained.

/// Name of the per-user global directory, resolved against `$HOME`.
pub const ZED_HOME_DIR_NAME: &str = ".zed-pkg";

/// Directory inside a project where installed packages appear
/// (`zed_modules/<org>/<name>`).
pub const MODULES_DIR: &str = "zed_modules";

/// Package manifest file name, at the repository root. TOML only.
pub const MANIFEST_FILE: &str = ".zpkg.toml";

/// Lockfile name, written next to the manifest.
pub const LOCKFILE_FILE: &str = ".zpkg.lock";

/// Optional file with extra publish-exclusion globs, one per line.
pub const IGNORE_FILE: &str = ".zedignore";

/// Store schema version segment; bump when the on-disk layout changes.
pub const STORE_VERSION: &str = "v1";

/// Directory inside an extracted store entry that holds the package files.
pub const STORE_PKG_DIR: &str = "pkg";

/// Root directory inside every artifact archive under which package files
/// are placed (like npm's `package/` prefix).
pub const ARCHIVE_ROOT: &str = "pkg";

/// Where `zed pack` writes artifacts, relative to the project root.
pub const PACK_OUT_DIR: &str = ".zed/pack";

/// Directory inside `zed_modules/` where package-declared executables are
/// hoisted (`zed_modules/.bin/<name>`), runnable via `zed run <name>`.
pub const BIN_DIR: &str = ".bin";

/// Store entry path for an artifact, relative to the zed home directory,
/// e.g. `store/v1/ab/abcdef.../`.
pub fn store_entry_rel(sha256: &str) -> String {
    let prefix = sha256.get(..2).unwrap_or("xx");
    format!("store/{STORE_VERSION}/{prefix}/{sha256}")
}

/// Build-cache entry for an artifact on one platform, relative to the zed
/// home directory, e.g. `builds/v1/macos-aarch64/ab/abcdef.../`. Source
/// extraction (`store/`) is platform-independent; compiled results are not,
/// so they cache separately per (sha256, platform) and the source store
/// stays immutable.
pub fn build_entry_rel(platform: &str, sha256: &str) -> String {
    let prefix = sha256.get(..2).unwrap_or("xx");
    format!("builds/{STORE_VERSION}/{platform}/{prefix}/{sha256}")
}

/// The `os-arch` pair used to key the build cache, e.g. `linux-x86_64`.
pub fn current_platform() -> String {
    format!("{}-{}", std::env::consts::OS, std::env::consts::ARCH)
}
