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

Integration tests needs local servers:
```
PROJECT_DIR=$PWD
mkdir -p server
cd ..
git clone https://github.com/Blockstream/electrs
cd electrs
git checkout 5bae341585f70699cf12b587a1e9d392df43d674
cargo install --debug --root $PROJECT_DIR/server/electrs_liquid --locked --path . --features liquid
cd $PROJECT_DIR/server
curl -L https://github.com/ElementsProject/elements/releases/download/elements-0.18.1.8/elements-0.18.1.8-x86_64-linux-gnu.tar.gz | tar -xvz elements-0.18.1.8/bin/elementsd
cd $PROJECT_DIR
```

To run them:
```
export ELECTRS_LIQUID_EXEC=$PWD/server/electrs_liquid/bin/electrs
export ELEMENTSD_EXEC=$PWD/server/elements-0.18.1.8/bin/elementsd

DEBUG=1 ./launch_integration_tests.sh liquid
DEBUG=1 ./launch_integration_tests.sh dex
```
