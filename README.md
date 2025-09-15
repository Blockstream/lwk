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
