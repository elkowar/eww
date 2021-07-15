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

        eww = naersk-lib.buildPackage {
          pname = "eww";
          nativeBuildInputs = with pkgs; [ pkg-config gtk3 ];
          root = ./.;
        };


      in
      rec {
        packages.eww = eww;

        defaultPackage = eww;

        apps.eww = flake-utils.lib.mkApp {
          drv = eww;
        };
        defaultApp = apps.eww;

        devShell = import ./shell.nix { inherit pkgs; };
      });
}
