# BEWallet

**WARNING: early stage software, DO NOT USE it with real funds**

BEWallet is a library For Bitcoin and Elements wallets.

BEWallet is based on [Blockstream's GDK](https://github.com/Blockstream/gdk).
Basically, it took all GDK Rust pieces and move it to their own project.

BEWallet uses Electrum backends.

To build:

```
./build_wally.sh
export WALLY_DIR=$PWD/libwally-core/build/lib/
cargo build
```

Run tests:

Integration tests needs local servers:
```
PROJECT_DIR=$PWD
mkdir -p server
cd ..
git clone https://github.com/Blockstream/electrs
cd electrs
git checkout 5bae341585f70699cf12b587a1e9d392df43d674
cargo install --debug --root $PROJECT_DIR/server/electrs_bitcoin --locked --path .
cargo install --debug --root $PROJECT_DIR/server/electrs_liquid --locked --path . --features liquid
cd $PROJECT_DIR/server
curl https://bitcoincore.org/bin/bitcoin-core-0.20.1/bitcoin-0.20.1-x86_64-linux-gnu.tar.gz | tar -xvz server/bitcoin-0.20.1/bin/bitcoind
curl -L https://github.com/ElementsProject/elements/releases/download/elements-0.18.1.8/elements-0.18.1.8-x86_64-linux-gnu.tar.gz | tar -xvz server/elements-0.18.1.8/bin/elementsd
cd $PROJECT_DIR
```

```
export ELECTRS_LIQUID_EXEC=$PWD/server/electrs_liquid/bin/electrs
export ELECTRS_EXEC=$PWD/server/electrs_bitcoin/bin/electrs
export BITCOIND_EXEC=$PWD/server/bitcoin-0.20.1/bin/bitcoind
export ELEMENTSD_EXEC=$PWD/server/elements-0.18.1.8/bin/elementsd

DEBUG=1 ./launch_integration_tests.sh bitcoin
DEBUG=1 ./launch_integration_tests.sh liquid
cd gdk_common
cargo test
cd ..
cd gdk_electrum
cargo test
cd ..
```
