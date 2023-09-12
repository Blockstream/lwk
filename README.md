# BEWallet

**WARNING: early stage software, DO NOT USE it with real funds**

BEWallet is a library for Elements wallets.

BEWallet is based on [Blockstream's GDK](https://github.com/Blockstream/gdk).
Essentially all GDK Rust pieces were moved to this project.

BEWallet uses Electrum backends.

To build:

```
cargo build
```

Run tests:

Run unit tests:
```
cargo test --lib
```

End to end tests needs local servers:

```
./download_bins.sh # needed once unless server binaries changes
. .envrc  # not needed if yoy use direnv and you executed `direnv allow`
```

To run end to end tests:

```
cargo test
```
