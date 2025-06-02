{
  description = "Rust development environment";
  inputs = {
    nixpkgs.url = "github:nixos/nixpkgs/nixos-25.05";
  };

  outputs = {nixpkgs, ...}: let
    system = "x86_64-linux";
    pkgs = nixpkgs.legacyPackages.${system};
  in {
    devShells.${system}.default = pkgs.mkShell {
      packages = with pkgs; [
        # rust-analyzer
        # cargo
        # clippy
        # rustfmt
        # rustc
        rustup
      ];
      shellHook = ''
        export SHELL=${pkgs.bashInteractive}/bin/bash
      '';
    };
  };
}
