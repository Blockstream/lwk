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

* [List transactions](./lwk_bindings/tests/bindings/list_transactions.py) of a wpkh/slip77 wallet, also compute the UTXO only balance
* [Send transaction](./lwk_bindings/tests/bindings/send_transaction.py) of a wpkh/slip77 wallet in a regtest environment
* [Send asset](./lwk_bindings/tests/bindings/send_asset.py) of a wpkh/slip77 wallet in a regtest environment
* [Issue a Liquid asset](./lwk_bindings/tests/bindings/issue_asset.py)
* [Custom persister](./lwk_bindings/tests/bindings/custom_persister.py), the caller code provide how the wallet updates are persisted
* [AMP0](./lwk_bindings/tests/bindings/amp0.py) demonstrates Asset Management Platform version 0 integration
* [AMP2](./lwk_bindings/tests/bindings/amp2.py) demonstrates Asset Management Platform protocol integration
* [External unblinding](./lwk_bindings/tests/bindings/external_unblind.py) shows how to unblind transaction data externally
* [LiquiDEX](./lwk_bindings/tests/bindings/liquidex.py) demonstrates Liquid decentralized swap functionality
* [Manual coin selection](./lwk_bindings/tests/bindings/manual_coin_selection.py) shows how to manually select coins for transactions
* [Multisig](./lwk_bindings/tests/bindings/multisig.py) demonstrates multisignature wallet setup and usage
* [PSET details](./lwk_bindings/tests/bindings/pset_details.py) shows how to inspect and work with Partially Signed Elements Transactions

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
