{
  description = "A devShell example";

  inputs = {
    nixpkgs.url      = "github:NixOS/nixpkgs/nixos-unstable";
    fenix = {
			url = "github:nix-community/fenix";
			inputs.nixpkgs.follows = "nixpkgs";
		};
    flake-utils.url  = "github:numtide/flake-utils";
  };

  outputs = { self, nixpkgs, fenix, flake-utils, ... }:
    flake-utils.lib.eachDefaultSystem (system:
      let
        overlays = [ fenix.overlays.default ];
        pkgs = import nixpkgs {
          inherit system overlays;
        };
        bInputs = with pkgs; [
          openssl
          pkg-config
        ];
      in
      with pkgs;
      {
        devShells.default = mkShell {
					nativeBuildInputs = with pkgs; [ rust-analyzer ];
          buildInputs = bInputs;
					LD_LIBRARY_PATH = "$LD_LIBRARY_PATH:${
              pkgs.lib.makeLibraryPath bInputs
            }";
        };
      }
    );
}

