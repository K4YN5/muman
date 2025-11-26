{
  description = "Muman: Local Music Manager";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    rust-overlay.url = "github:oxalica/rust-overlay";
    flake-utils.url = "github:numtide/flake-utils";
  };

  outputs = { self, nixpkgs, rust-overlay, flake-utils, ... }:
    flake-utils.lib.eachDefaultSystem (system:
      let
        overlays = [ (import rust-overlay) ];
        pkgs = import nixpkgs {
          inherit system overlays;
        };
        
        # Helper to get the version from Cargo.toml
        manifest = (pkgs.lib.importTOML ./Cargo.toml).package;
      in
      {
        packages.default = pkgs.rustPlatform.buildRustPackage {
          pname = manifest.name;
          version = manifest.version;
          
          src = ./.;

          # This requires Cargo.lock to be committed to git!
          cargoLock = {
            lockFile = ./Cargo.lock;
          };

          nativeBuildInputs = [ pkgs.pkg-config ];
          buildInputs = [ pkgs.openssl ]; 
        };

        # This allows you to run 'nix develop' to get a coding environment
        devShells.default = pkgs.mkShell {
          nativeBuildInputs = [ pkgs.pkg-config ];
          buildInputs = [ 
            pkgs.cargo 
            pkgs.rustc 
            pkgs.openssl 
            pkgs.rust-analyzer
          ];
        };
      }
    );
}
