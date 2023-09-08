# BEWallet

**WARNING: early stage software, DO NOT USE it with real funds**

BEWallet is a library for Elements wallets.

BEWallet is based on [Blockstream's GDK](https://github.com/Blockstream/gdk).
Basically, it took all GDK Rust pieces and moved them to their own project.

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
mkdir -p bin
cd bin

# electrs
wget https://github.com/RCasatta/electrsd/releases/download/electrs_releases/electrs_linux_esplora_a33e97e1a1fc63fa9c20a116bb92579bbf43b254_liquid.gz
gunzip electrs_linux_esplora_a33e97e1a1fc63fa9c20a116bb92579bbf43b254_liquid.gz
chmod +x electrs_linux_esplora_a33e97e1a1fc63fa9c20a116bb92579bbf43b254_liquid
export ELECTRS_LIQUID_EXEC=$(realpath electrs_linux_esplora_a33e97e1a1fc63fa9c20a116bb92579bbf43b254_liquid)

# elementsd
wget https://github.com/ElementsProject/elements/releases/download/elements-0.18.1.12/elements-0.18.1.12-x86_64-linux-gnu.tar.gz
tar -xzf elements-0.18.1.12-x86_64-linux-gnu.tar.gz
export ELEMENTSD_EXEC=$(realpath elements-0.18.1.12/bin/elementsd)
```

If you use direnv a convenient `.envrc` is in the repo (after inspection needs `direnv allow`)

To run end to end tests:

```
cargo test
```
