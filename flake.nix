{
  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    flake-utils.url = "github:numtide/flake-utils";
    rust-overlay = {
      url = "github:oxalica/rust-overlay";
      inputs = {
        nixpkgs.follows = "nixpkgs";
        flake-utils.follows = "flake-utils";
      };
    };
    crane = {
      url = "github:ipetkov/crane";
      inputs = {
        nixpkgs.follows = "nixpkgs";
      };
    };
    electrs-flake = {
      url = "github:blockstream/electrs";
      inputs = {
        nixpkgs.follows = "nixpkgs";
        flake-utils.follows = "flake-utils";
        crane.follows = "crane";
        rust-overlay.follows = "rust-overlay";
      };
    };
  };
  outputs = { self, nixpkgs, flake-utils, rust-overlay, crane, electrs-flake }:
    flake-utils.lib.eachDefaultSystem
      (system:
        let
          overlays = [ (import rust-overlay) ];
          pkgs = import nixpkgs {
            inherit system overlays;
          };

          rustToolchain = pkgs.pkgsBuildHost.rust-bin.fromRustupToolchainFile ./rust-toolchain.toml;

          craneLib = (crane.mkLib pkgs).overrideToolchain rustToolchain;

          electrs = electrs-flake.apps.${system}.blockstream-electrs-liquid;

          #src = craneLib.cleanCargoSource ./.; # rust specific, but filters out md files, which are included with include_str for doc purpose
          src = nixpkgs.lib.cleanSource ./.;

          nativeBuildInputs = with pkgs; [ rustToolchain pkg-config ]; # required only at build time
          buildInputs = [ pkgs.openssl pkgs.udev ]; # also required at runtime

          commonArgs = {
            inherit src buildInputs nativeBuildInputs;

            # the following must be kept in sync with the ones in ./lwk_cli/Cargo.toml
            # note there should be a way to read those from there with
            # craneLib.crateNameFromCargoToml { cargoToml = ./path/to/Cargo.toml; }
            # but I can't make it work
            pname = "lwk_cli";
            version = "0.3.0";
          };
          cargoArtifacts = craneLib.buildDepsOnly commonArgs;
          # remember, `set1 // set2` does a shallow merge:
          bin = craneLib.buildPackage (commonArgs // {
            inherit cargoArtifacts;
            cargoTestExtraArgs = "--lib"; # only unit testing, integration testing has more requirements (docker and other executables)
          });

        in
        {
          packages =
            {
              # that way we can build `bin` specifically,
              # but it's also the default.
              inherit bin;
              default = bin;
            };

          devShells.default = pkgs.mkShell {
            inputsFrom = [ bin ];

            buildInputs = [ ];

            ELEMENTSD_EXEC = "${pkgs.elements}/bin/elementsd";
            ELECTRS_LIQUID_EXEC = electrs.program;

          };
        }
      );
}

