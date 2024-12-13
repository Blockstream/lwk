# Liquid Wallet Kit

![LWK logo](docs/logos/web/LWK_logo_white_on_dark_rgb.png)

**NOTE: LWK is in public beta and still undergoing significant development. Use it at your own risk.**

## What is Liquid Wallet Kit (LWK)?

**LWK** is a collection of Rust crates for [Liquid](https://liquid.net) Wallets.
Its goal is to provide all the necessary building blocks for Liquid wallet development to enable a broad range of use cases on Liquid.

By not following a monolithic approach but instead providing a group of function-specific libraries, LWK allows us to offer a modular, flexible and ergonomic toolset for Liquid development. This design lets application developers pick only what they need and focus on the relevant aspects of their use cases.

We want LWK to be a reference tool driven both by Blockstream and Liquid participants that helps make Liquid integration frictionless, define ecosystem standards and leverage advanced Liquid capabilities such as covenants or swaps.

While LWK is Rust native, we provide [bindings](./lwk_bindings) for Python, Kotlin and Swift using [Mozilla UniFFI](https://mozilla.github.io/uniffi-rs/) and we provide preliminary support for [WASM](./lwk_wasm). We will continue polishing these bindings and expanding the available options.
Additionally, the Bull Bitcoin team has developed [Dart/Flutter](https://github.com/SatoshiPortal/lwk-dart) bindings.


## Main Features

* **Watch-Only** wallet support: using Liquid descriptors, better known as
  [CT descriptors](https://github.com/ElementsProject/ELIPs/blob/main/elip-0150.mediawiki).
* **PSET** based: transactions are shared and processed using the
  [Partially Signed Elements Transaction](https://github.com/ElementsProject/elements/blob/1fcf0cf2323b7feaff5d1fc4c506fff5ec09132e/doc/pset.mediawiki) format.
* **Electrum** and **Esplora** [backends](https://github.com/Blockstream/electrs):
  no need to run and sync a full Liquid node or rely on closed source servers.
* **Asset issuance**, **reissuance** and **burn** support: manage the lifecycle
  of your Issued Assets with a lightweight client.
* **Generic multisig** wallets: create a wallet controlled by
  any combination of hardware or software signers, with a user
  specified quorum.
* **Hardware signer** support: receive, issue, reissue and burn L-BTC and
  Issued Assets with your hardware signer, using singlesig or multisig
  wallets (currently [**Jade**](https://blockstream.com/jade/) only, with more coming soon).
* **Native bindings** [PoC support](./lwk_bindings#readme) for Python, Kotlin and Swift, with many other language available soon using [uniffi](https://mozilla.github.io/uniffi-rs/)
* **WASM** [`lwk_wasm`](./lwk_wasm) crate, see it in action in the [Liquid Web Wallet](https://liquidwebwallet.org/).
* **JSON-RPC Server** support: all functions are exposed via JSON-RPC Server, making it easier to build your own frontend, GUI, or integration.

## LWK Structure

LWK functionalities are split into different component crates that might be useful independently.

* [`lwk_cli`](./lwk_cli): a CLI tool to use LWK wallets.
* [`lwk_wollet`](./lwk_wollet): library for watch-only wallets;
  specify a CT descriptor, generate new addresses, get balance,
  create PSETs and other actions.
* [`lwk_signer`](./lwk_signer): interact with Liquid signers
  to get your PSETs signed.
* [`lwk_jade`](./lwk_jade): unlock Jade, get xpubs,
  register multisig wallets, sign PSETs and more.
* [`lwk_bindings`](./lwk_bindings): use LWK from other languages.
* [`lwk_wasm`](./lwk_wasm): use LWK from WebAssembly.
* and more:
  common or ancillary components ([`lwk_common`](./lwk_common),
  [`lwk_rpc_model`](./lwk_rpc_model), [`lwk_tiny_rpc`](./lwk_tiny_rpc),
  [`lwk_app`](./lwk_app)),
  future improvements ([`lwk_hwi`](./lwk_hwi)),
  testing infrastructure ([`lwk_test_util`](./lwk_test_util),
  [`lwk_containers`](./lwk_containers))

For instance, mobile app devs might be interested mainly in
`lwk_bindings`, `lwk_wollet` and `lwk_signer`.
While backend developers might want to directly use `lwk_cli`
in their systems.

Internal crate dependencies are shown in this diagram: an arrow indicates "depends on" (when dotted the dependency is feature-activated, when blue is a dev-dependency):

![Dep tree](docs/dep-tree.svg)

(generated with `cargo depgraph --workspace-only --dev-deps`)

## Getting started with LWK Development

### Rust

#### Build

You can build all crates with:
```shell
cargo build
```

Or you can build a single crate with:
```shell
cargo build -p lwk_wollet
```

#### Rust Examples

* [Create a testnet watch-only wallet from a CT wallet descriptor and get list of transactions](./lwk_wollet/examples/list_transactions.rs)
* Documentation for all Rust crates is available at [docs.rs](https://docs.rs/releases/search?query=lwk)

### Python

#### Install from PyPI

```shell
pip install lwk
```

#### Build Python wheel

First, create a virtual env, skip the step if you already created it.

```shell
cd lwk/lwk_bindings
virtualenv venv
source venv/bin/activate
pip install maturin maturin[patchelf] uniffi-bindgen
```

Then build the wheel

```shell
cd lwk/lwk_bindings
maturin develop
```

Try it (note there is still an issue in how we import the package when using the wheel):

```python
import lwk
str(lwk.Network.mainnet())
```

#### Python Examples

* [List transactions](./lwk_bindings/tests/bindings/list_transactions.py) of a wpkh/slip77 wallet
* [Send transaction](./lwk_bindings/tests/bindings/send_transaction.py) of a wpkh/slip77 wallet in a regtest environment
* [Send asset](./lwk_bindings/tests/bindings/send_asset.py) of a wpkh/slip77 wallet in a regtest environment
* [Issue a Liquid asset](./lwk_bindings/tests/bindings/issue_asset.py)
* [Custom persister](./lwk_bindings/tests/bindings/custom_persister.py), the caller code provide how the wallet updates are persisted

### Kotlin

#### Build

This will build the bindings library in debug mode and generate the kotlin file

```shell
just kotlin
```

Create android bindings library libs, 4 architectures in release mode

```shell
just android
```

#### Kotlin Examples

* [List transactions](./lwk_bindings/tests/bindings/list_transactions.kts) of a wpkh/slip77 wallet

### Swift

#### Swift Examples

* [List transactions](./lwk_bindings/tests/bindings/list_transactions.swift) of a wpkh/slip77 wallet


### C#

#### C# Examples

C# bindings use dotnet SDK 6.0, they are very immature at the moment:

- They use a uniffi bindings generator from a [third party](https://github.com/NordSecurity/uniffi-bindgen-cs) which didn't yet ship for uniffi 0.28 
- It's currently tested only in linux
- The dynamic library is referenced in a non-standard way

* [List transactions](./lwk_bindings/tests/bindings/list_transactions.cs) of a wpkh/slip77 wallet

### WASM

We currently provide preliminary support but are committed to continue working on this to have a fully featured LWK working on WASM environments.
[See these instructions to try out LWK on WASM](./lwk_wasm)

## See what LWK is capable of by using the command line tool (LWK_CLI)

All LWK functions are exposed via a local JSON-RPC server that communicates with a CLI tool so you can see LWK in action.

This JSON-RPC Server also makes it easier to build your own frontend, GUI, or integration.

If you want to see an overview of LWK and a demo with the CLI tool check out this [video](https://community.liquid.net/c/videos/demo-liquid-wallet-kit-lwk)

### Installing LWK_CLI from crates.io

```sh
$ cargo install lwk_cli
```
or if you want to connect Jade over serial:

```sh
$ cargo install lwk_cli --features serial
```

### Building LWK_CLI from source

First you need [rust](https://www.rust-lang.org/tools/install), our MSRV is 1.78.0
then you can build from source:

```sh
$ git clone git@github.com:Blockstream/lwk.git
$ cd lwk
$ cargo install --path ./lwk_cli/
```

Or
```
$ cargo install --path ./lwk_cli/ --features serial
```
To enable connection with Jade over serial.

## Using LWK_CLI

Help will show available commands:

```sh
$ lwk_cli --help
```

Start the rpc server (default in Liquid Testnet)
and put it in background
```sh
$ lwk_cli server start
```
Every command requires the server running so open a new shell to run the client.

Create new BIP39 mnemonic for a software signer
```sh
$ lwk_cli signer generate
```
Load a software *signer* named `sw` from a given BIP39 mnemonic
```sh
$ lwk_cli signer load-software --signer sw --persist false --mnemonic "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about"
```

Create a p2wpkh *wallet* named `ss` (install [`jq`](https://github.com/jqlang/jq) or extract the descriptor manually)
```sh
$ DESC=$(lwk_cli signer singlesig-desc -signer sw --descriptor-blinding-key slip77 --kind wpkh | jq -r .descriptor)
$ lwk_cli wallet load --wallet ss -d $DESC
```

Get the wallet balance
```sh
$ lwk_cli wallet balance -w ss
```
If you have a Jade, you can plug it in and use it to create a
wallet and sign its transactions.

Probe connected Jades and prompt user to unlock it to get identifiers needed to load Jade on LWK

```sh
$ lwk_cli signer jade-id
```
Load Jade using returned ID

```sh
$ lwk_cli signer load-jade --signer <SET_A_NAME_FOR_THIS_JADE> --id <ID>
```
Get xpub from loaded Jade

```sh
$ lwk_cli signer xpub --signer <NAME_OF_THIS_JADE> --kind <bip84, bip49 or bip87>
```

When you're done, stop the rpc server.
```sh
$ lwk_cli server stop
```

## Tests

Run unit tests:
```
cargo test --lib
```

End-to-end tests need some local servers:

```
./context/download_bins.sh # needed once unless server binaries changes
. .envrc  # not needed if you use direnv and you executed `direnv allow`
```

And also the following docker images:

```
docker pull xenoky/local-jade-emulator:1.0.27
docker pull tulipan81/blind_pin_server:v0.0.7
```

Note: Failed test executions can leave docker containers running. To stop all running containers run:

```
docker stop $(docker ps -a -q)
```

To run end-to-end tests:

```
cargo test
```

To see log outputs use `RUST_LOG` for example

```
RUST_LOG=info cargo test -- test_name
RUST_LOG=jade=debug cargo test -- test_name  # filter only on specific module
```

### Test with a physical Jade

Tests using Jade over serial (via USB cable) need an additional dependency:
```
apt install -y libudev-dev
```

These serial tests cannot be executed in parallel, so we need the `--test-threads 1` flag.
```
cargo test -p lwk_jade --features serial -- serial --include-ignored --test-threads 1
cargo test -p lwk_wollet --features serial -- serial --include-ignored --test-threads 1
```

## Docs

To generate documentation you can use

```
RUSTDOCFLAGS="--cfg docsrs" cargo +nightly doc --all-features --no-deps --open
```

## Nix

We provide a flake for a dev environment and for running the `lwk_cli`.
If you use direnv and allow the `.envrc` the dev environment is automatically loaded
as soon as you enter the directory, otherwise you can run:

```
nix develop
```

To run `lwk_cli` on nix-enabled system:

```
nix run github:blockstream/lwk
```

## History

BEWallet was [originally](https://github.com/LeoComandini/BEWallet/)
an Elements/Liquid wallet library written in Rust to develop
prototypes and experiments.

BEWallet was based on [Blockstream's GDK](https://github.com/Blockstream/gdk).
Essentially some GDK Rust pieces were moved to this project.

This was used as the starting point for the Liquid Wallet Kit project.
Parts that were not necessary have been dropped,
many things have been polished, and new features have been added.

The codebase has been entirely re-written, and now it has
almost no similarity with the original code.
