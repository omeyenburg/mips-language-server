{ pkgs ? import <nixpkgs> {} }:

pkgs.mkShell {
  name = "Rust development environment";

  buildInputs = [
    pkgs.rustup
  ];
}
