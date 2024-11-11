{
  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-23.11";
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
    registry-flake = {
      url = "github:blockstream/asset_registry/flake";
      inputs = {
        nixpkgs.follows = "nixpkgs";
        flake-utils.follows = "flake-utils";
        crane.follows = "crane";
        rust-overlay.follows = "rust-overlay";
      };
    };
  };
  outputs = { self, nixpkgs, flake-utils, rust-overlay, crane, electrs-flake, registry-flake }:
    flake-utils.lib.eachDefaultSystem
      (system:
        let
          overlays = [ (import rust-overlay) ];
          pkgs = import nixpkgs {
            inherit system overlays;
          };
          inherit (pkgs) lib;

          rustToolchain = pkgs.pkgsBuildHost.rust-bin.fromRustupToolchainFile ./rust-toolchain.toml;

          craneLib = (crane.mkLib pkgs).overrideToolchain rustToolchain;

          electrs = electrs-flake.apps.${system}.blockstream-electrs-liquid;
          registry = registry-flake.packages.${system};

          # When filtering sources, we want to allow assets other than .rs files
          src = lib.cleanSourceWith {
            src = ./.; # The original, unfiltered source
            filter = path: type:
              (lib.hasSuffix "\.elf" path) ||
              (lib.hasSuffix "\.json" path) ||
              (lib.hasSuffix "\.md" path) ||
              (lib.hasInfix "/test_data/" path) ||
              (lib.hasInfix "/test/data/" path) ||
              (lib.hasInfix "/tests/data/" path) || # TODO unify these dir names

              # Default filter from crane (allow .rs files)
              (craneLib.filterCargoSources path type)
            ;
          };

          nativeBuildInputs = with pkgs; [ rustToolchain pkg-config ]; # required only at build time
          buildInputs = [ pkgs.openssl pkgs.udev ]; # also required at runtime

          commonArgs = {
            inherit src buildInputs nativeBuildInputs;

            # the following must be kept in sync with the ones in ./lwk_cli/Cargo.toml
            # note there should be a way to read those from there with
            # craneLib.crateNameFromCargoToml { cargoToml = ./path/to/Cargo.toml; }
            # but I can't make it work
            pname = "lwk_cli";
            version = "0.8.0";
          };
          cargoArtifacts = craneLib.buildDepsOnly commonArgs;
          # remember, `set1 // set2` does a shallow merge:
          bin = craneLib.buildPackage (commonArgs // {
            inherit cargoArtifacts;
            cargoTestExtraArgs = "--lib"; # only unit testing, integration testing has more requirements (docker and other executables)

            # Without the following also libs are included in the package, and we need to produce only the executable.
            # There should probably a way to avoid creating it in the first place, but for now this works.
            postInstall = ''
              rm -r $out/lib
            '';
          });

          elementsd_ct = pkgs.elementsd.overrideAttrs (final: prev: rec {
            version = "23.2.4";

            src = pkgs.fetchFromGitHub {
              owner = "ElementsProject";
              repo = "elements";
              rev = "elements-${version}";
              sha256 = "sha256-UNjYkEZBjGuhkwBxSkNXjBBcLQqoan/afCLhoR2lOY4=";
            };
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

            buildInputs = [ registry.bin rustToolchain ];

            ELEMENTSD_EXEC = "${elementsd_ct}/bin/elementsd";
            BITCOIND_EXEC = "${pkgs.bitcoind}/bin/bitcoind";
            ELECTRS_LIQUID_EXEC = electrs.program;
            SKIP_VERIFY_DOMAIN_LINK = "1"; # the registry server skips validation
          };
        }
      );
}

