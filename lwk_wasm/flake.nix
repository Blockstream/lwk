{
  description = "wasm-pack setup";

  inputs = {
    nixpkgs = { url = "github:nixos/nixpkgs/nixos-unstable"; };
    rust-overlay = {
      url = "github:oxalica/rust-overlay";
      inputs = {
        nixpkgs.follows = "nixpkgs";
      };
    };
  };

  outputs = { nixpkgs, rust-overlay, ... }:
    let system = "x86_64-linux";
    in {
      devShell.${system} =
        let
          pkgs = import nixpkgs {
            inherit system;
            overlays = [ rust-overlay.overlay ];
          };
        in
        (({ pkgs, ... }:
          pkgs.mkShell {
            buildInputs = with pkgs; [
              cargo
              wasm-pack
              clang
              (rust-bin.stable.latest.default.override {
                extensions = [ "rust-src" ];
                targets = [ "wasm32-unknown-unknown" ];
              })
            ];

            CC_wasm32_unknown_unknown = "clang-17";
            CFLAGS_wasm32_unknown_unknown = "-I${pkgs.clang_17}/resource-root/include";

          }) { pkgs = pkgs; });
    };
}
