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
          rustToolchain = pkgs.pkgsBuildHost.rust-bin.fromRustupToolchainFile ../rust-toolchain.toml;
        in
        (({ pkgs, ... }:
          pkgs.mkShell {
            buildInputs = with pkgs; [
              cargo
              wasm-pack
              clang_15
              nodejs_22
              rustToolchain
            ];

            CC_wasm32_unknown_unknown = "clang-15";
            CFLAGS_wasm32_unknown_unknown = "-I${pkgs.clang_15}/resource-root/include";
            RUSTFLAGS = "--cfg=web_sys_unstable_apis";

          }) { pkgs = pkgs; });
    };
}
