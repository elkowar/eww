with import <nixpkgs> { };

mkShell rec {
  name = "eww";

  buildInputs = [ glib rustup cairo atk gdk-pixbuf pango gtk3 ];
  nativeBuildInputs = [ pkgconfig wrapGAppsHook ];
}
