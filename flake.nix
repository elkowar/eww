{
  inputs = {
	fenix = {
      url = "github:nix-community/fenix";
      inputs.nixpkgs.follows = "nixpkgs";
    };
    nixpkgs.url = "nixpkgs/nixpkgs-unstable";
    flake-utils.url = "github:numtide/flake-utils";
  };
  outputs = {flake-utils, fenix, nixpkgs}:
    flake-utils.lib.eachDefaultSystem (system: 
	let pkgs = nixpkgs.legacyPackages.${system} // { inherit (fenix.packages.${system}.latest) cargo rustc; } ; 

	in
	{
	devShell = import ./shell.nix {inherit pkgs;};
    });
}
