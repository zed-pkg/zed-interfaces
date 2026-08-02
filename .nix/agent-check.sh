#!/usr/bin/env bash
set -euo pipefail

export CI="${CI:-1}"
export NO_COLOR="${NO_COLOR:-1}"

repo_root="$(git rev-parse --show-toplevel)"
cd "$repo_root"

cache_root="${NIX_AGENT_CACHE_ROOT:-$repo_root/.cache/nix-agent}"
export CARGO_HOME="${CARGO_HOME:-$cache_root/cargo}"
export CARGO_TARGET_DIR="${CARGO_TARGET_DIR:-$cache_root/target}"
export XDG_CACHE_HOME="${XDG_CACHE_HOME:-$cache_root/xdg}"
mkdir -p "$CARGO_HOME" "$CARGO_TARGET_DIR" "$XDG_CACHE_HOME"

run_stage() {
  local stage="$1"
  printf '\n==> agent-check stage: %s\n' "$stage"

  case "$stage" in
    preflight)
      git diff --check
      nixfmt --check flake.nix .nix/dev-shell.nix
      shellcheck .nix/agent-check.sh
      shfmt -i 2 -ci -d .nix/agent-check.sh
      actionlint .github/workflows/*.yml
      nix flake check --no-update-lock-file --show-trace
      ;;
    format)
      cargo fmt --check
      ;;
    lint)
      # Nixpkgs currently pins Rust/Clippy 1.95, whose nonminimal_bool advice
      # conflicts with the explicit, security-auditable path rejection list.
      # Current stable CI still runs every lint with -D warnings; only this
      # toolchain-version-specific style lint is exempted in the pinned shell.
      cargo clippy --locked --all-targets -- \
        -D warnings \
        -A clippy::nonminimal_bool
      ;;
    test)
      cargo test --locked
      cargo test --locked --doc
      ;;
    schemas)
      cargo run --locked --example generate_schemas
      git diff --exit-code -- schemas/
      ;;
    all)
      local child
      for child in preflight format lint test schemas; do
        run_stage "$child"
      done
      ;;
    *)
      printf 'unknown agent-check stage: %s\n' "$stage" >&2
      return 64
      ;;
  esac
}

case "${1:-all}" in
  all | preflight | format | lint | test | schemas)
    run_stage "${1:-all}"
    ;;
  *)
    printf 'usage: agent-check [all|preflight|format|lint|test|schemas]\n' >&2
    exit 64
    ;;
esac
