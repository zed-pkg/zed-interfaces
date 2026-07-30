//! Language and ecosystem tagging for multi-language packages.
//!
//! A repository that generates API clients for many languages cannot ship as
//! one zed package: a Java consumer would download the Node, Python and Dart
//! trees it will never load, and nothing would stop a Node project from
//! installing the Java client and silently getting an unusable
//! `zed_modules` entry. Such a repo instead publishes one package **per
//! language**, all at one version — `acme/acme-clients-java@1.1.2`,
//! `acme/acme-clients-nodejs@1.1.2`, … — and each declares what it is for.
//!
//! Two axes are needed, and conflating them breaks one case or the other:
//!
//! * [`Language`] **names** the package. Java, Kotlin, Scala and Clojure are
//!   four distinct clients a human asks for by name, so they are four packages.
//! * [`Ecosystem`] decides **who may install it** and how the toolchain sees
//!   it. Those same four languages are one ecosystem ([`Ecosystem::Jvm`]):
//!   consumed off a classpath, and all equally valid in a Gradle project.
//!
//! Keying only on language would leave the install guard unable to tell that a
//! Kotlin package belongs in a Gradle build. Keying only on ecosystem would
//! collapse `-java` and `-kotlin` into one name. Hence both, with
//! [`Language::ecosystem`] supplying the default mapping so a manifest normally
//! declares just `language`.
//!
//! Both default to `universal`, which is never gated — every manifest written
//! before this module existed keeps its old meaning.

use std::collections::BTreeSet;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// The language a package's code is written for. Names the published package
/// (`<repo>-<language>`), and implies an [`Ecosystem`] via
/// [`Language::ecosystem`].
///
/// Canonical tokens are the colloquial names people search for — `nodejs`, not
/// `node`; `golang`, not `go` — because that is what ends up in a package name
/// a human has to recall. The shorter spellings, and near-synonyms like
/// `typescript`, are accepted by [`Language::from_token`] and by umbrella
/// variant aliases, so both spellings resolve.
#[derive(
    Debug,
    Clone,
    Copy,
    Default,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Hash,
    Serialize,
    Deserialize,
    JsonSchema,
)]
#[serde(rename_all = "lowercase")]
pub enum Language {
    /// Not language-specific: protocol schemas, docs, `.proto` files, an
    /// umbrella package. Never subject to the install ecosystem guard.
    #[default]
    Universal,
    C,
    Clojure,
    Cpp,
    Crystal,
    Csharp,
    Dart,
    Elixir,
    Erlang,
    /// Flutter is a framework rather than a language, but a Flutter client is a
    /// separate package from a plain Dart one (different `pubspec.yaml`,
    /// different deps), so it gets its own token.
    Flutter,
    Fsharp,
    Gleam,
    Golang,
    Haskell,
    Java,
    Julia,
    Kotlin,
    Lua,
    Matlab,
    Nim,
    /// Covers JavaScript and TypeScript alike: for a client library they are
    /// one artifact published to one ecosystem, so `typescript`, `javascript`,
    /// `ts`, `js` and `node` all resolve here rather than splitting the package.
    Nodejs,
    Ocaml,
    Php,
    Powershell,
    Python,
    R,
    Ruby,
    Rust,
    /// Rust compiled to WebAssembly. Distinct from [`Language::Rust`] because
    /// the artifact is consumed by a JS bundler, not by Cargo.
    #[serde(rename = "rust-wasm")]
    RustWasm,
    Scala,
    Shell,
    Swift,
    Zig,
}

/// How a consumer's build system takes a dependency in: the resolution
/// mechanism, not the language. Drives the install guard and which toolchain
/// wiring file `zed install` writes.
#[derive(
    Debug,
    Clone,
    Copy,
    Default,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Hash,
    Serialize,
    Deserialize,
    JsonSchema,
)]
#[serde(rename_all = "lowercase")]
pub enum Ecosystem {
    /// No ecosystem constraint. Installs into any project; never gated.
    #[default]
    Universal,
    Cargo,
    Cmake,
    Composer,
    Cran,
    Gem,
    Gomod,
    Hackage,
    /// Elixir, Erlang and Gleam share Hex and the BEAM.
    Hex,
    /// Anything consumed off a JVM classpath: Java, Kotlin, Scala, Clojure.
    Jvm,
    Julia,
    Luarocks,
    Matlab,
    Nimble,
    Npm,
    Nuget,
    Opam,
    Psgallery,
    /// Dart and Flutter (`pub`).
    Pub,
    Pypi,
    Shards,
    Shell,
    Swiftpm,
    Zig,
}

impl Language {
    pub fn as_str(&self) -> &'static str {
        use Language::*;
        match self {
            Universal => "universal",
            C => "c",
            Clojure => "clojure",
            Cpp => "cpp",
            Crystal => "crystal",
            Csharp => "csharp",
            Dart => "dart",
            Elixir => "elixir",
            Erlang => "erlang",
            Flutter => "flutter",
            Fsharp => "fsharp",
            Gleam => "gleam",
            Golang => "golang",
            Haskell => "haskell",
            Java => "java",
            Julia => "julia",
            Kotlin => "kotlin",
            Lua => "lua",
            Matlab => "matlab",
            Nim => "nim",
            Nodejs => "nodejs",
            Ocaml => "ocaml",
            Php => "php",
            Powershell => "powershell",
            Python => "python",
            R => "r",
            Ruby => "ruby",
            Rust => "rust",
            RustWasm => "rust-wasm",
            Scala => "scala",
            Shell => "shell",
            Swift => "swift",
            Zig => "zig",
        }
    }

    /// True for the default (`universal`); lets manifests omit the field.
    pub fn is_default(&self) -> bool {
        matches!(self, Language::Universal)
    }

    /// The ecosystem this language publishes into by default. A package may
    /// override it (`[package] ecosystem = "..."`) when the language does not
    /// determine consumption — a `rust-wasm` client is consumed by a JS
    /// bundler, but a Rust `cdylib` of the same source would be Cargo.
    pub fn ecosystem(&self) -> Ecosystem {
        use Language::*;
        match self {
            Universal => Ecosystem::Universal,
            C | Cpp => Ecosystem::Cmake,
            Clojure | Java | Kotlin | Scala => Ecosystem::Jvm,
            Crystal => Ecosystem::Shards,
            Csharp | Fsharp => Ecosystem::Nuget,
            Dart | Flutter => Ecosystem::Pub,
            Elixir | Erlang | Gleam => Ecosystem::Hex,
            Golang => Ecosystem::Gomod,
            Haskell => Ecosystem::Hackage,
            Julia => Ecosystem::Julia,
            Lua => Ecosystem::Luarocks,
            Matlab => Ecosystem::Matlab,
            Nim => Ecosystem::Nimble,
            // wasm-pack output is a JS package, not a Cargo one.
            Nodejs | RustWasm => Ecosystem::Npm,
            Ocaml => Ecosystem::Opam,
            Php => Ecosystem::Composer,
            Powershell => Ecosystem::Psgallery,
            Python => Ecosystem::Pypi,
            R => Ecosystem::Cran,
            Ruby => Ecosystem::Gem,
            Rust => Ecosystem::Cargo,
            Shell => Ecosystem::Shell,
            Swift => Ecosystem::Swiftpm,
            Zig => Ecosystem::Zig,
        }
    }

    /// Resolve a user-supplied token — a canonical name, a common short form,
    /// or a source-directory name — to a language. This is what makes
    /// `zed add …-go` find `…-golang`, and what lets a repo whose directory is
    /// `clients/ts/` publish as `-nodejs`.
    ///
    /// Case- and separator-insensitive: `RustWasm`, `rust_wasm` and
    /// `rust-wasm` are the same token.
    pub fn from_token(token: &str) -> Option<Language> {
        use Language::*;
        let normalized: String = token
            .trim()
            .to_ascii_lowercase()
            .chars()
            .map(|c| if c == '_' || c == ' ' { '-' } else { c })
            .collect();
        Some(match normalized.as_str() {
            "universal" | "any" | "none" => Universal,
            "c" => C,
            "clojure" | "clj" => Clojure,
            "cpp" | "c++" | "cxx" => Cpp,
            "crystal" => Crystal,
            "csharp" | "c#" | "cs" | "dotnet" | "net" => Csharp,
            "dart" => Dart,
            "elixir" | "ex" => Elixir,
            "erlang" | "erl" => Erlang,
            "flutter" => Flutter,
            "fsharp" | "f#" | "fs" => Fsharp,
            "gleam" => Gleam,
            "golang" | "go" => Golang,
            "haskell" | "hs" => Haskell,
            "java" => Java,
            "julia" | "jl" => Julia,
            "kotlin" | "kt" => Kotlin,
            "lua" => Lua,
            "matlab" => Matlab,
            "nim" => Nim,
            "nodejs" | "node" | "javascript" | "js" | "typescript" | "ts" => Nodejs,
            "ocaml" | "ml" => Ocaml,
            "php" => Php,
            "powershell" | "pwsh" | "ps1" => Powershell,
            "python" | "py" => Python,
            "r" => R,
            "ruby" | "rb" => Ruby,
            "rust" | "rs" => Rust,
            "rust-wasm" | "rustwasm" | "wasm" => RustWasm,
            "scala" => Scala,
            "shell" | "sh" | "bash" => Shell,
            "swift" => Swift,
            "zig" => Zig,
            _ => return None,
        })
    }

    /// Parse a stored value, degrading unknown input to `universal` rather than
    /// failing a read. Mirrors [`crate::version::VersionScheme::from_str_lenient`]:
    /// a value written by a newer zed must never wedge an older one.
    pub fn from_str_lenient(s: &str) -> Language {
        Language::from_token(s).unwrap_or(Language::Universal)
    }
}

impl std::fmt::Display for Language {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

impl Ecosystem {
    pub fn as_str(&self) -> &'static str {
        use Ecosystem::*;
        match self {
            Universal => "universal",
            Cargo => "cargo",
            Cmake => "cmake",
            Composer => "composer",
            Cran => "cran",
            Gem => "gem",
            Gomod => "gomod",
            Hackage => "hackage",
            Hex => "hex",
            Jvm => "jvm",
            Julia => "julia",
            Luarocks => "luarocks",
            Matlab => "matlab",
            Nimble => "nimble",
            Npm => "npm",
            Nuget => "nuget",
            Opam => "opam",
            Psgallery => "psgallery",
            Pub => "pub",
            Pypi => "pypi",
            Shards => "shards",
            Shell => "shell",
            Swiftpm => "swiftpm",
            Zig => "zig",
        }
    }

    /// True for the default (`universal`); lets manifests omit the field.
    pub fn is_default(&self) -> bool {
        matches!(self, Ecosystem::Universal)
    }

    pub fn from_token(token: &str) -> Option<Ecosystem> {
        use Ecosystem::*;
        Some(match token.trim().to_ascii_lowercase().as_str() {
            "universal" | "any" | "none" => Universal,
            "cargo" | "crates" | "crates-io" => Cargo,
            "cmake" => Cmake,
            "composer" => Composer,
            "cran" => Cran,
            "gem" | "rubygems" => Gem,
            "gomod" | "go" | "goproxy" => Gomod,
            "hackage" => Hackage,
            "hex" => Hex,
            "jvm" | "maven" | "gradle" => Jvm,
            "julia" => Julia,
            "luarocks" => Luarocks,
            "matlab" => Matlab,
            "nimble" => Nimble,
            "npm" | "node" | "nodejs" | "yarn" | "pnpm" => Npm,
            "nuget" | "dotnet" => Nuget,
            "opam" => Opam,
            "psgallery" | "powershell" => Psgallery,
            "pub" | "dart" | "flutter" => Pub,
            "pypi" | "pip" | "python" => Pypi,
            "shards" => Shards,
            "shell" | "sh" => Shell,
            "swiftpm" | "spm" => Swiftpm,
            "zig" => Zig,
            _ => return None,
        })
    }

    pub fn from_str_lenient(s: &str) -> Ecosystem {
        Ecosystem::from_token(s).unwrap_or(Ecosystem::Universal)
    }

    /// Filenames in a project root that identify this ecosystem. A leading
    /// `*.` matches by extension; everything else is an exact filename.
    ///
    /// Empty for ecosystems with no conventional marker file
    /// ([`Ecosystem::Matlab`], [`Ecosystem::Shell`]) — those are never
    /// auto-detected, so a project consuming one must say so explicitly with
    /// `[install] adapter` or `--adapter`.
    pub fn marker_files(&self) -> &'static [&'static str] {
        use Ecosystem::*;
        match self {
            Universal | Matlab | Shell => &[],
            Cargo => &["Cargo.toml"],
            Cmake => &["CMakeLists.txt"],
            Composer => &["composer.json"],
            Cran => &["DESCRIPTION"],
            Gem => &["Gemfile", "*.gemspec"],
            Gomod => &["go.mod", "go.work"],
            Hackage => &["stack.yaml", "*.cabal"],
            Hex => &["mix.exs", "rebar.config", "gleam.toml"],
            Jvm => &[
                "pom.xml",
                "build.gradle",
                "build.gradle.kts",
                "build.sbt",
                "deps.edn",
                "project.clj",
            ],
            Julia => &["Project.toml", "JuliaProject.toml"],
            Luarocks => &["*.rockspec"],
            Nimble => &["*.nimble"],
            Npm => &["package.json"],
            Nuget => &["*.csproj", "*.fsproj", "*.sln"],
            Opam => &["dune-project", "*.opam"],
            Psgallery => &["*.psd1"],
            Pub => &["pubspec.yaml"],
            Pypi => &[
                "pyproject.toml",
                "setup.py",
                "setup.cfg",
                "requirements.txt",
            ],
            Shards => &["shard.yml"],
            Swiftpm => &["Package.swift"],
            Zig => &["build.zig.zon", "build.zig"],
        }
    }

    /// Every ecosystem, for exhaustive detection and for error messages that
    /// list what a project could be.
    pub const ALL: &'static [Ecosystem] = &[
        Ecosystem::Cargo,
        Ecosystem::Cmake,
        Ecosystem::Composer,
        Ecosystem::Cran,
        Ecosystem::Gem,
        Ecosystem::Gomod,
        Ecosystem::Hackage,
        Ecosystem::Hex,
        Ecosystem::Jvm,
        Ecosystem::Julia,
        Ecosystem::Luarocks,
        Ecosystem::Matlab,
        Ecosystem::Nimble,
        Ecosystem::Npm,
        Ecosystem::Nuget,
        Ecosystem::Opam,
        Ecosystem::Psgallery,
        Ecosystem::Pub,
        Ecosystem::Pypi,
        Ecosystem::Shards,
        Ecosystem::Shell,
        Ecosystem::Swiftpm,
        Ecosystem::Zig,
    ];

    /// True when `filename` is one of this ecosystem's markers.
    pub fn matches_marker(&self, filename: &str) -> bool {
        self.marker_files().iter().any(|pattern| {
            match pattern.strip_prefix("*.") {
                // Extension pattern: `*.gemspec` matches `acme.gemspec` but not
                // a file literally named `.gemspec`.
                Some(ext) => filename
                    .rsplit_once('.')
                    .is_some_and(|(stem, e)| !stem.is_empty() && e == ext),
                None => filename == *pattern,
            }
        })
    }
}

impl std::fmt::Display for Ecosystem {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

/// Identify every ecosystem present in a project root, given its top-level
/// filenames. Returns a set because polyglot repos are normal — a Rust service
/// with a TypeScript frontend is both `cargo` and `npm`, and a dependency for
/// either belongs there.
///
/// Takes filenames rather than a path so it stays pure and trivially testable;
/// callers in `zed-cli` supply a directory listing.
pub fn detect_ecosystems<'a, I>(filenames: I) -> BTreeSet<Ecosystem>
where
    I: IntoIterator<Item = &'a str>,
{
    let names: Vec<&str> = filenames.into_iter().collect();
    Ecosystem::ALL
        .iter()
        .filter(|eco| names.iter().any(|name| eco.matches_marker(name)))
        .copied()
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_language_round_trips_through_its_own_token() {
        // Guards the three parallel match arms (as_str / from_token /
        // ecosystem): adding a variant to one and forgetting another is the
        // easy mistake, and it would silently mis-tag a published package.
        for lang in [
            Language::Universal,
            Language::C,
            Language::Clojure,
            Language::Cpp,
            Language::Crystal,
            Language::Csharp,
            Language::Dart,
            Language::Elixir,
            Language::Erlang,
            Language::Flutter,
            Language::Fsharp,
            Language::Gleam,
            Language::Golang,
            Language::Haskell,
            Language::Java,
            Language::Julia,
            Language::Kotlin,
            Language::Lua,
            Language::Matlab,
            Language::Nim,
            Language::Nodejs,
            Language::Ocaml,
            Language::Php,
            Language::Powershell,
            Language::Python,
            Language::R,
            Language::Ruby,
            Language::Rust,
            Language::RustWasm,
            Language::Scala,
            Language::Shell,
            Language::Swift,
            Language::Zig,
        ] {
            assert_eq!(
                Language::from_token(lang.as_str()),
                Some(lang),
                "`{lang}` does not parse back from its own as_str()"
            );
        }
    }

    #[test]
    fn language_serde_uses_the_canonical_token() {
        // The manifest is the wire format; a rename here silently invalidates
        // every published manifest.
        let json = serde_json::to_string(&Language::RustWasm).unwrap();
        assert_eq!(json, "\"rust-wasm\"");
        assert_eq!(
            serde_json::from_str::<Language>("\"nodejs\"").unwrap(),
            Language::Nodejs
        );
        assert_eq!(
            serde_json::to_string(&Language::Golang).unwrap(),
            "\"golang\""
        );
    }

    #[test]
    fn colloquial_and_short_tokens_resolve_to_one_package() {
        // The whole point of the alias table: a user who types `-go` or `-ts`
        // must land on the package that actually exists.
        for token in ["go", "golang", "GoLang"] {
            assert_eq!(Language::from_token(token), Some(Language::Golang));
        }
        for token in ["node", "nodejs", "js", "javascript", "ts", "typescript"] {
            assert_eq!(Language::from_token(token), Some(Language::Nodejs));
        }
        assert_eq!(Language::from_token("rust_wasm"), Some(Language::RustWasm));
        assert_eq!(Language::from_token("  Python  "), Some(Language::Python));
        assert_eq!(Language::from_token("cobol"), None);
    }

    #[test]
    fn unknown_tokens_degrade_to_universal_rather_than_failing() {
        // A manifest written by a newer zed must not wedge an older one.
        assert_eq!(Language::from_str_lenient("brainfuck"), Language::Universal);
        assert_eq!(Ecosystem::from_str_lenient("bazel"), Ecosystem::Universal);
    }

    #[test]
    fn jvm_languages_share_one_ecosystem_but_keep_distinct_names() {
        // This is the reason for two axes. Same ecosystem => a Kotlin client
        // installs fine in a Gradle project. Distinct languages => `-java` and
        // `-kotlin` are separately installable packages.
        for lang in [
            Language::Java,
            Language::Kotlin,
            Language::Scala,
            Language::Clojure,
        ] {
            assert_eq!(lang.ecosystem(), Ecosystem::Jvm);
        }
        assert_ne!(Language::Java.as_str(), Language::Kotlin.as_str());
    }

    #[test]
    fn rust_wasm_is_consumed_by_npm_not_cargo() {
        assert_eq!(Language::Rust.ecosystem(), Ecosystem::Cargo);
        assert_eq!(Language::RustWasm.ecosystem(), Ecosystem::Npm);
    }

    #[test]
    fn universal_maps_to_universal_so_untagged_packages_are_never_gated() {
        assert_eq!(Language::Universal.ecosystem(), Ecosystem::Universal);
        assert!(Language::default().is_default());
        assert!(Ecosystem::default().is_default());
        assert!(Ecosystem::Universal.marker_files().is_empty());
    }

    #[test]
    fn detects_a_single_ecosystem_from_its_marker() {
        assert_eq!(
            detect_ecosystems(["package.json", "README.md"]),
            BTreeSet::from([Ecosystem::Npm])
        );
        assert_eq!(
            detect_ecosystems(["build.gradle.kts"]),
            BTreeSet::from([Ecosystem::Jvm])
        );
        assert_eq!(
            detect_ecosystems(["go.mod"]),
            BTreeSet::from([Ecosystem::Gomod])
        );
    }

    #[test]
    fn detects_every_ecosystem_in_a_polyglot_project() {
        // A Rust service with a TS frontend is both; a dependency for either
        // one belongs here, so the guard must not pick a single winner.
        let found = detect_ecosystems(["Cargo.toml", "package.json", "pyproject.toml"]);
        assert_eq!(
            found,
            BTreeSet::from([Ecosystem::Cargo, Ecosystem::Npm, Ecosystem::Pypi])
        );
    }

    #[test]
    fn extension_markers_match_by_suffix_only() {
        assert_eq!(
            detect_ecosystems(["acme-client.gemspec"]),
            BTreeSet::from([Ecosystem::Gem])
        );
        assert_eq!(
            detect_ecosystems(["Acme.Client.csproj"]),
            BTreeSet::from([Ecosystem::Nuget])
        );
        // A bare dotfile named after the extension is not a project marker.
        assert!(detect_ecosystems([".gemspec"]).is_empty());
        assert!(detect_ecosystems(["gemspec"]).is_empty());
    }

    #[test]
    fn detects_nothing_in_a_directory_with_no_markers() {
        // Must stay empty rather than guessing: the guard treats "no detected
        // ecosystem" as "cannot verify", not as "wrong ecosystem".
        assert!(detect_ecosystems(["README.md", "LICENSE"]).is_empty());
        assert!(detect_ecosystems(Vec::<&str>::new()).is_empty());
    }

    #[test]
    fn all_covers_every_non_default_ecosystem() {
        // A new ecosystem missing from ALL would never be detected, so the
        // guard would silently pass everything in such a project.
        assert!(!Ecosystem::ALL.contains(&Ecosystem::Universal));
        for lang in [
            Language::Java,
            Language::Nodejs,
            Language::Golang,
            Language::Python,
            Language::Rust,
            Language::Dart,
            Language::Ruby,
            Language::Php,
            Language::Swift,
            Language::Csharp,
            Language::Gleam,
            Language::Zig,
        ] {
            assert!(
                Ecosystem::ALL.contains(&lang.ecosystem()),
                "{lang} maps to {} which is missing from ALL",
                lang.ecosystem()
            );
        }
    }
}
