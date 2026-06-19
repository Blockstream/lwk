{
  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    flake-utils.url = "github:numtide/flake-utils";
    rust-overlay = {
      url = "github:oxalica/rust-overlay";
      inputs = {
        nixpkgs.follows = "nixpkgs";
      };
    };
    crane = {
      url = "github:ipetkov/crane";
    };
    waterfalls-nixpkgs.url = "github:NixOS/nixpkgs/c5296fdd05cfa2c187990dd909864da9658df755";
    waterfalls-crane.url = "github:ipetkov/crane/0314e365877a85c9e5758f9ea77a9972afbb4c21";
    waterfalls-rust-overlay = {
      url = "github:oxalica/rust-overlay/e9bcd12156a577ac4e47d131c14dc0293cc9c8c2";
      inputs.nixpkgs.follows = "waterfalls-nixpkgs";
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
    waterfalls-flake = {
      url = "github:RCasatta/waterfalls";
      inputs.nixpkgs.follows = "waterfalls-nixpkgs";
      inputs.crane.follows = "waterfalls-crane";
      inputs.rust-overlay.follows = "waterfalls-rust-overlay";
      inputs.flake-utils.follows = "flake-utils";
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
    # TODO: Remove nixpkgs-mdbook once MSRV reach 1.88+ and we can upgrade mdbook dependency in docs/snippets/processor
    nixpkgs-mdbook.url = "github:NixOS/nixpkgs/566e53c2ad750c84f6d31f9ccb9d00f823165550";
  };
  outputs =
    {
      self,
      nixpkgs,
      flake-utils,
      rust-overlay,
      crane,
      electrs-flake,
      waterfalls-flake,
      registry-flake,
      nixpkgs-mdbook,
      ...
    }:
    flake-utils.lib.eachDefaultSystem (
      system:
      let
        overlays = [ (import rust-overlay) ];
        pkgs = import nixpkgs {
          inherit system overlays;
        };
        pkgs-mdbook = import nixpkgs-mdbook {
          inherit system;
        };
        inherit (pkgs) lib;

        rustToolchain = pkgs.pkgsBuildHost.rust-bin.fromRustupToolchainFile ./rust-toolchain.toml;

        craneLib = (crane.mkLib pkgs).overrideToolchain rustToolchain;

        electrs = electrs-flake.apps.${system}.blockstream-electrs-liquid;
        registry = registry-flake.packages.${system};
        waterfalls = waterfalls-flake.packages.${system}.default;

        # When filtering sources, we want to allow assets other than .rs files
        src = lib.cleanSourceWith {
          src = ./.; # The original, unfiltered source
          filter =
            path: type:
            (lib.hasSuffix "\.elf" path)
            || (lib.hasSuffix "\.json" path)
            || (lib.hasSuffix "\.md" path)
            || (lib.hasInfix "/test_data/" path)
            || (lib.hasInfix "/test/data/" path)
            || (lib.hasInfix "/tests/data/" path)
            # TODO unify these dir names
            ||

              # Default filter from crane (allow .rs files)
              (craneLib.filterCargoSources path type);
        };

        nativeBuildInputs = with pkgs; [
          rustToolchain
          pkg-config
        ]; # required only at build time
        buildInputs = [
          pkgs.openssl
          pkgs.udev
        ]; # also required at runtime

        commonArgs = {
          inherit src buildInputs nativeBuildInputs;

          # the following must be kept in sync with the ones in ./lwk_cli/Cargo.toml
          # note there should be a way to read those from there with
          # craneLib.crateNameFromCargoToml { cargoToml = ./path/to/Cargo.toml; }
          # but I can't make it work
          pname = "lwk_cli";
          version = "0.18.0";
        };
        cargoArtifacts = craneLib.buildDepsOnly commonArgs;
        # remember, `set1 // set2` does a shallow merge:
        bin = craneLib.buildPackage (
          commonArgs
          // {
            inherit cargoArtifacts;
            cargoTestExtraArgs = "--lib"; # only unit testing, integration testing has more requirements (docker and other executables)

            # Without the following also libs are included in the package, and we need to produce only the executable.
            # There should probably a way to avoid creating it in the first place, but for now this works.
            postInstall = ''
              rm -r $out/lib
            '';
          }
        );
        amp2MockArgs = commonArgs // {
          pname = "amp2_mock";
          cargoExtraArgs = "-p amp2_mock";
        };
        amp2MockCargoArtifacts = craneLib.buildDepsOnly amp2MockArgs;
        amp2Mock = craneLib.buildPackage (
          amp2MockArgs
          // {
            cargoArtifacts = amp2MockCargoArtifacts;

            postInstall = ''
              rm -rf $out/lib
            '';
          }
        );

        # Build mdbook-snippets from local source
        mdbook-snippets = craneLib.buildPackage {
          src = lib.cleanSource ./docs/snippets/processor;
          inherit nativeBuildInputs;
          buildInputs = [ ];

          pname = "mdbook-snippets";
          version = "0.1.0";
        };

        mcp-language-server = pkgs.mcp-language-server;

      in
      {
        packages = {
          # that way we can build `bin` specifically,
          # but it's also the default.
          inherit bin;
          default = bin;
          amp2-mock = amp2Mock;
          inherit mdbook-snippets;
          inherit mcp-language-server;
        };

        devShells.default = pkgs.mkShell {
          inputsFrom = [ bin ];

          buildInputs = [
            mcp-language-server
            registry.bin
            rustToolchain
            pkgs.websocat
            pkgs.heaptrack
            pkgs-mdbook.mdbook
            pkgs-mdbook.mdbook-mermaid
            mdbook-snippets
            pkgs.cargo-depgraph
            pkgs.cargo-bloat
            pkgs.cargo-nextest
            amp2Mock
            pkgs.grcov
            pkgs.go
            pkgs.lsof
            pkgs.nixfmt
            pkgs.psmisc
            pkgs.sqlite
            pkgs.libxml2
            pkgs.yq # to validate yml
            pkgs.jq
          ];

          ELEMENTSD_EXEC = "${pkgs.elementsd}/bin/elementsd";
          BITCOIND_EXEC = "${pkgs.bitcoind}/bin/bitcoind";
          ELECTRS_LIQUID_EXEC = electrs.program;
          WATERFALLS_EXEC = "${waterfalls}/bin/waterfalls";
          ASSET_REGISTRY_EXEC = "${registry.default}/bin/server";
          AMP2_MOCK_EXEC = "${amp2Mock}/bin/amp2_mock";
          WEBSOCAT_EXEC = "${pkgs.websocat}/bin/websocat";
          SKIP_VERIFY_DOMAIN_LINK = "1"; # the registry server skips validation
        };
      }
    );
}
