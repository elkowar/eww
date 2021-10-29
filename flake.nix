{
  inputs = {
    flake-compat = {
      url = "github:edolstra/flake-compat";
      flake = false;
    };
    nixpkgs.url = "nixpkgs/nixpkgs-unstable";
    flake-utils.url = "github:numtide/flake-utils";
    fenix = {
      url = "github:nix-community/fenix";
      inputs.nixpkgs.follows = "nixpkgs";
    };
    naersk.url = "github:nmattia/naersk";
  };
  outputs = { self, flake-utils, fenix, nixpkgs, naersk, flake-compat, ... }:
    flake-utils.lib.eachDefaultSystem (system:
      let
        # Add rust nightly to pkgs
        pkgs = nixpkgs.legacyPackages.${system} // { inherit (fenix.packages.${system}.latest) cargo rustc rust-src clippy-preview rustfmt-preview; };

        naersk-lib = (naersk.lib."${system}".override {
          cargo = pkgs.cargo;
          rustc = pkgs.rustc;
        });

        eww = { wayland ? false }:
          naersk-lib.buildPackage {
            pname = "eww";
            buildInputs = pkgs.lib.optional wayland pkgs.gtk-layer-shell;
            nativeBuildInputs = with pkgs; [ pkg-config gtk3 ];
            cargoBuildOptions = opts: opts ++ pkgs.lib.optionals wayland [ "--no-default-features" "--features=wayland" ];
            root = ./.;
          };

      in rec {
        packages.eww = eww {};
        packages.eww-wayland = eww {wayland=true;};

        defaultPackage = self.packages.${system}.eww;

        apps.eww = flake-utils.lib.mkApp { drv = packages.eww; };
        apps.eww-wayland = flake-utils.lib.mkApp { drv = packages.eww-wayland; };
        defaultApp = apps.eww;

        devShell = pkgs.mkShell {
          packages = with pkgs; [
            rustc
            cargo
            rust-analyzer
            gcc
            gtk3
            gtk-layer-shell
            pkg-config
            rustfmt-preview
            clippy-preview
            deno
            mdbook
          ];
          RUST_SRC_PATH = "${pkgs.rust-src}/lib/rustlib/src/rust/library";
        };
      });
}
