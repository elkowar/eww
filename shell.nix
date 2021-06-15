with import <nixpkgs> { };

let
  moz_overlay = import (builtins.fetchTarball
    "https://github.com/mozilla/nixpkgs-mozilla/archive/master.tar.gz");
  nixpkgs = import <nixpkgs> { overlays = [ moz_overlay ]; };
in mkShell rec {
  name = "eww";

  buildInputs = [
    glib
    cairo
    atk
    gdk-pixbuf
    pango
    gtk3
    nixpkgs.latest.rustChannels.nightly.rust
    zola
    nodejs
  ];
  nativeBuildInputs = [ pkgconfig wrapGAppsHook ];

  RUST_SRC_PATH = "${nixpkgs.rust.packages.stable.rustPlatform.rustLibSrc}";
}
