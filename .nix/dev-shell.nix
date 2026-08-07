{
  pkgs,
  agentCheck,
  toolchain,
}:
pkgs.mkShell {
  packages = toolchain ++ [ agentCheck ];

  LANG = if pkgs.stdenv.hostPlatform.isDarwin then "en_US.UTF-8" else "C.UTF-8";
  LC_ALL = if pkgs.stdenv.hostPlatform.isDarwin then "en_US.UTF-8" else "C.UTF-8";

  shellHook = ''
    export NIX_DEV_SHELL=zed-interfaces
    export NIX_AGENT_CACHE_ROOT="''${NIX_AGENT_CACHE_ROOT:-$PWD/.cache/nix-agent}"
    export CARGO_HOME="''${CARGO_HOME:-$NIX_AGENT_CACHE_ROOT/cargo}"
    export CARGO_TARGET_DIR="''${CARGO_TARGET_DIR:-$NIX_AGENT_CACHE_ROOT/target}"
    export XDG_CACHE_HOME="''${XDG_CACHE_HOME:-$NIX_AGENT_CACHE_ROOT/xdg}"
    mkdir -p "$CARGO_HOME" "$CARGO_TARGET_DIR" "$XDG_CACHE_HOME"
  '';
}
