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

