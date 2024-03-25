{ pkgs ? import <nixpkgs> {} }:
  pkgs.mkShell {
    # nativeBuildInputs is usually what you want -- tools you need to run
    nativeBuildInputs = with pkgs.buildPackages; [ 
      rustup
      clang 
      pkg-config
      udev
      openssl
    ];
    
    OPENSSL_DEV=pkgs.openssl.dev;
    RUSTFLAGS="--cfg=web_sys_unstable_apis";
}
