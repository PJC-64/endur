{
  description = "Endur build and development environment";

  # Provides abstraction to boiler-code when specifying multi-platform outputs.
  inputs = {
    flake-utils.url = "github:numtide/flake-utils";
    nixpkgs.url = "github:nixos/nixpkgs/nixos-unstable";
    rust-overlay.url = "github:oxalica/rust-overlay";
  };

  outputs = { self, nixpkgs, flake-utils, rust-overlay, ... }:
    flake-utils.lib.eachDefaultSystem (system:
      let
        shortRev = if (self ? shortRev) then self.shortRev else "dev-${self.lastModifiedDate}";

        pkgs = import nixpkgs {
          inherit system;
          overlays = [ rust-overlay.overlay ];
        };

        endur = pkgs.rustPlatform.buildRustPackage {
          pname = "endur";
          version = "${shortRev}";
          description = "A background process that saves uncommited changes on git";

          src = self;

          cargoLock = {
            lockFile = self + "/Cargo.lock";
          };

          buildInputs = [
            pkgs.openssl
          ];

          nativeBuildInputs = [
            pkgs.rust-bin.stable.latest.minimal
            pkgs.pkg-config
          ];

          ENDUR_VERSION_SUFFIX = "${shortRev}";
        };

        packages = flake-utils.lib.flattenTree {
          inherit endur;
        };

        apps = {
          endur = flake-utils.lib.mkApp { drv = packages.endur; };
        };
      in
      rec {
        defaultPackage = packages.endur;
        defaultApp = apps.endur;
        devShell = pkgs.mkShell {
          ENDUR_VERSION_SUFFIX = endur.version;
          RUST_SRC_PATH = "${pkgs.rust.packages.stable.rustPlatform.rustLibSrc}";

          buildInputs = [
            pkgs.openssl
            pkgs.pkgconfig
            (pkgs.rust-bin.stable.latest.default.override { extensions = [ "rust-src" ]; })
          ];
        };

      });
}
