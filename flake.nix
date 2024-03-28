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
  };
  outputs = { self, nixpkgs, flake-utils, rust-overlay, crane }:
    flake-utils.lib.eachDefaultSystem
      (system:
        let
          overlays = [ (import rust-overlay) ];
          pkgs = import nixpkgs {
            inherit system overlays;
          };
          rustToolchain = pkgs.pkgsBuildHost.rust-bin.fromRustupToolchainFile ./rust-toolchain.toml;

          craneLib = (crane.mkLib pkgs).overrideToolchain rustToolchain;

          #src = craneLib.cleanCargoSource ./.; # rust specific, but filters out md files, which are included with include_str for doc purpose
          src = nixpkgs.lib.cleanSource ./.;

          nativeBuildInputs = with pkgs; [ rustToolchain pkg-config ]; # required only at build time
          buildInputs = with pkgs; [ openssl udev ]; # also required at runtime

          commonArgs = {
            inherit src buildInputs nativeBuildInputs;
          };
          cargoArtifacts = craneLib.buildDepsOnly commonArgs;
          # remember, `set1 // set2` does a shallow merge:
          bin = craneLib.buildPackage (commonArgs // {
            inherit cargoArtifacts;
            doCheck = false;  # use cargoTestExtraArgs = "--lib"; once there is no e2e tests in lib and once .elf files are not removed from cleanSource
          });

        in
        with pkgs;
        {
          packages =
            {
              # that way we can build `bin` specifically,
              # but it's also the default.
              inherit bin;
              default = bin;
            };

          devShells.default = mkShell {
            inputsFrom = [ bin ];
            
            buildInputs = with pkgs; [ wasm-pack ];
            
            ELEMENTSD_EXEC = "${pkgs.elements}/bin/elementsd";
            
            BAR="ciaociao";
          };
        }
      );
}
