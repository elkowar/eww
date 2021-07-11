{ pkgs ? import <nixpkgs> {
    overlays = [
      (import (fetchTarball
        "https://github.com/nix-community/fenix/archive/main.tar.gz"))
    ];
  }
}:

pkgs.mkShell {
  packages = with pkgs; [
    rustc
    cargo
    rust-analyzer
    gcc
    gtk3
    pkg-config
    rustfmt
    clippy
  ];


  RUST_SRC_PATH = "${pkgs.rust.packages.stable.rustPlatform.rustLibSrc}";
}
