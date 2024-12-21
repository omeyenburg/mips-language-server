{
  description = "Development environment with bear, OpenGL, SDL, and clangd";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    flake-utils.url = "github:numtide/flake-utils";
  };

  outputs = { self, nixpkgs, flake-utils }:
    flake-utils.lib.eachDefaultSystem (system:
      let
        pkgs = nixpkgs.legacyPackages.${system};
      in {
        devShells.default = pkgs.mkShell {
          buildInputs = with pkgs; [
            cargo
            rustup
          ];

          # Set up environment variables for OpenGL and SDL
          shellHook = ''
            export PS1="\[\033[1;32m\][nix-dev:\w]\$ \[\033[0m\]"
            echo "> Development Environment Activated"
          '';
        };
      }
    );
}

# vim: sw=2 sts=2 et
