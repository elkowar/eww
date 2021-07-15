{ pkgs ? import <nixpkgs> {
    overlays = [
      (import (fetchTarball
      "https://github.com/nix-community/fenix/archive/main.tar.gz"))
      (self: super: {
          rustc = super.fenix.latest.rustc;
          cargo  = super.fenix.latest.cargo;
          rust-src = super.fenix.latest.rust-src;
      }
        )
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
    rustfmt-preview
    clippy-preview
    deno
    mdbook
  ];


  RUST_SRC_PATH = "${pkgs.rust-src}/lib/rustlib/src/rust/library";
}
