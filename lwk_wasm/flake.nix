{
  description = "wasm-pack setup";

  inputs = {
    lwk-flake = {
      url = "path:..";
    };
    nixpkgs.follows = "lwk-flake/nixpkgs";
    rust-overlay.follows = "lwk-flake/rust-overlay";
  };

  outputs = { nixpkgs, rust-overlay, lwk-flake, ... }:
    let system = "x86_64-linux";
    in {
      devShell.${system} =
        let
          pkgs = import nixpkgs {
            inherit system;
            overlays = [ rust-overlay.overlays.default ];
          };
          inherit (pkgs) lib;
          rustToolchain = pkgs.pkgsBuildHost.rust-bin.fromRustupToolchainFile ../rust-toolchain.toml;
          clang = pkgs.llvmPackages_21.clang-unwrapped;
        in
        (({ pkgs, ... }:
          pkgs.mkShell {
            buildInputs = with pkgs; [
              cargo
              wasm-pack
              nodejs_22
              rustToolchain
              clang
            ];

            CC_wasm32_unknown_unknown = "${lib.getExe clang}";
            CFLAGS_wasm32_unknown_unknown =
              "-I${clang.lib}/lib/clang/${lib.versions.major clang.version}/include";
            RUSTFLAGS = "--cfg=web_sys_unstable_apis";

          }) { pkgs = pkgs; });
    };
}
