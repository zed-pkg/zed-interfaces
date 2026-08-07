{
  description = "Agent-first development environment for the zed-pkg interface contract";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
  };

  outputs =
    { self, nixpkgs, ... }:
    let
      systems = [
        "aarch64-darwin"
        "aarch64-linux"
        "x86_64-darwin"
        "x86_64-linux"
      ];
      forAllSystems = nixpkgs.lib.genAttrs systems;
      pkgsFor = system: import nixpkgs { inherit system; };
      toolchainFor =
        pkgs: with pkgs; [
          actionlint
          bash
          cargo
          clippy
          git
          nix
          nixfmt
          rustc
          rustfmt
          shellcheck
          shfmt
        ];
    in
    {
      formatter = forAllSystems (system: (pkgsFor system).nixfmt);

      packages = forAllSystems (
        system:
        let
          pkgs = pkgsFor system;
          agentCheck = pkgs.writeShellApplication {
            name = "agent-check";
            runtimeInputs = toolchainFor pkgs;
            text = builtins.readFile ./.nix/agent-check.sh;
          };
        in
        {
          inherit agentCheck;
          default = agentCheck;
        }
      );

      apps = forAllSystems (system: {
        agent-check = {
          type = "app";
          program = "${self.packages.${system}.agentCheck}/bin/agent-check";
        };
        default = self.apps.${system}.agent-check;
      });

      checks = forAllSystems (system: {
        agentCheck = self.packages.${system}.agentCheck;
      });

      devShells = forAllSystems (
        system:
        let
          pkgs = pkgsFor system;
        in
        {
          default = import ./.nix/dev-shell.nix {
            inherit pkgs;
            agentCheck = self.packages.${system}.agentCheck;
            toolchain = toolchainFor pkgs;
          };
        }
      );
    };
}
