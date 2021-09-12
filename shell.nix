{npkgs ? import <nixpkgs> {}}:

let
    fenix = import
      (fetchTarball "https://github.com/nix-community/fenix/archive/main.tar.gz")
      { };
in
  let
    rustc = fenix.latest.rustc;
    cargo  = fenix.latest.cargo;
    rust-src = fenix.latest.rust-src;
    rustfmt-preview = fenix.latest.rustfmt-preview;
    clippy-preview = fenix.latest.clippy-preview;
  in


    npkgs.mkShell {
      packages = [
          rustc
          cargo
          npkgs.rust-analyzer
          npkgs.gcc
          npkgs.gtk3
          rust-src
          npkgs.pkg-config
          rustfmt-preview
          clippy-preview
          npkgs.deno
          npkgs.mdbook
        ];
      RUST_SRC_PATH = "${rust-src}/lib/rustlib/src/rust/library";
    }
