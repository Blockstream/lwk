# Changelog

## Unreleased

* Added `Amp2::is_registered()`.
* Breaking: `Amp2::new()` takes a `network` parameter.
* Removed `Amp2::new_testnet()`, use `Amp2::new()`.
* Python wheels no longer include the CycloneDX SBOM of the Rust dependencies (`lwk-*.dist-info/sboms/lwk_bindings.cyclonedx.json`), because it contains a build timestamp and a random serial number that prevent reproducible wheels.
* Added the `proxy` field to `EsploraClientBuilder` and `WaterfallsClientBuilder`, to send the requests through a proxy, e.g. Tor with `socks5h://127.0.0.1:9050`
* Breaking: Upgrade UniFFI 0.29.4 -> 0.31.2. Bindings and native library must come from the same release, since UniFFI 0.31 changed method checksums. Built-in trait exports (`Display`, `Hash`, `Eq`) are now generated deterministically, which is required for reproducible builds.
  * C#: generated with uniffi-bindgen-cs 0.11.1. Record properties are now PascalCase (e.g. `FirstName` instead of `first_name`), object handles are `ulong` instead of `IntPtr`.
  * Go: generated with uniffi-bindgen-go 0.7.1.
  * C++: generated with uniffi-bindgen-cpp 0.10.0.
  * Python wheels: generated with uniffi-bindgen 0.31.0.
  * Kotlin Multiplatform: generated with a gobley build ported to UniFFI 0.31.2 ([gobley#289](https://github.com/gobley/gobley/pull/289)).
* Kotlin Multiplatform: the Android libraries are built in release mode. They were unoptimized debug builds with debug assertions, and the `arm64-v8a` `liblwk.so` shrinks from 44.1 MB to 24.7 MB
* Breaking: If a PSET has a non-default sighash, `Signer.sign()` and `SecretKey.sign()` now return an error. To sign with non-default sighashes, pass `SignOptions.allow_sighash_single_anyonecanpay()` or `SignOptions.dangerous_allow_any_sighash()` to `Signer.sign_with_options()` or `SecretKey.sign_with_options()`
* Added `AddressResult::derivation_path()` and `Wollet::change()`

## 0.20.0

* Expose `WaterfallsClient.subscribe_reconnecting()` with server events and connection status updates in foreign bindings.
* `Signer.sign()` now returns an error if no signatures have been added.
* Update to `elements` 0.27. `LwkError` conversions from the previous hex error types were replaced by the new ones.
* Kotlin Multiplatform: fixed `UniFFI API checksum mismatch` on 32-bit ARM Android once the app is AOT-compiled, by generating the bindings with a gobley build that reads `u8`/`u16` return values correctly ([gobley#252](https://github.com/gobley/gobley/issues/252))
* `LwkTestEnv` and `LwkTestStore` are now behind the `test_env` feature. By default `lwk_test_util` is no longer a dependency of `lwk_bindings`, so shipping libraries do not include the test environment. Enable `test_env` to build and use the test helpers.

## 0.19.1

* Added `DescriptorPublicKey`
* Added `Signer::ss_desc()`
* Renamed `DerivationPath::from_account()` to `DerivationPath::ss_path()`
* Replaced `WolletDescriptor::from_xpub()` with `WolletDescriptor::ss_desc_from_external_signer()`
* Replaced `Amp2::elip153_from_str()` with `Amp2::elip153_from_external_signer()`
* Replaced `Signer::keyorigin_xpub_from_path()` with `Signer::derive_xpub()`

## 0.19.0

* Added `DerivationPath`
* Removed `get_path()`, replaced with `DerivationPath::from_account()`
* Changed "simplicity" functions `derive_keypair()`, `simplicity_derive_xonly_pubkey()`, `create_p2pk_signature()` to accept `DerivatioPath` instead of `str`
* Added `has_txs()` to `ElectrumClient`, `EsploraClient` and `WaterfallsClient`, to cheaply check if a descriptor has transaction history without a full scan
* Fixed Boltz chain-swap restoration to use the binding session network instead of Regtest

## 0.18.3

## 0.18.2

* Add `WaterfallsClient::subscribe()` and `WaterfallsSubscription` to expose Waterfalls descriptor subscription events to foreign bindings.

## 0.18.0

* deprecated `PsetBalance::fee()`, replacement `PsetBalance::fees()` and `PsetBalance::fees_in(asset_id)` ([ELIP-0204](https://github.com/ElementsProject/ELIPs/blob/main/elip-0204.mediawiki)).
* `SimplicityProgram::load()` no longer includes debug symbols, which changes the resulting CMR and address; use `SimplicityProgram::load_with_debug_symbols(source, args, true)` to restore the previous behaviour.

## 0.17.2

* `Network::to_string()` changed output for regtest variant from "ElementsRegtest { policy_asset: <32 byte hex>}" to "CustomElements(ElementsParams { policy_asset: <32 byte hex>, genesis_hash: <32 byte hex> })".

## 0.17.0

* deprecated `Txid::new()`, replacement `Txid::from_string()`
* deprecated `Txid::bytes()`, replacement `Txid:to_bytes()`
* deprecated `Script::new()`, replacement `Script::from_string()`
* deprecated `Script::bytes()`, replacement `Script:to_bytes()`
* removed `Hex` (struct): functions returning it will return `String` instead, and functions accepting it will accept `&str` instead. This should not affect end user languages.

## 0.16.0

* uniffi upgrade 0.28.2 -> 0.29.4

## 0.15.0

* removed `ForeignPersister` (trait), replacement `ForeignStore`
* removed `ForeignPersisterLink` (concrete "trait"), replacement `ForeignStoreLink`
* removed `Wollet::with_custom_persister()`, added `Wollet::with_custom_store()`
* `WolletDescriptor::url_encoded_descriptor` now returns a `Result`
* deprecated `Transaction::new()`, replacement `Transaction::from_string()`
* deprecated `Transaction::bytes()`, replacement `Transaction::to_bytes()`
* deprecated `TxOutSecrets::asset_bf()`, replacement `TxOutSecrets::asset_blinding_factor()`
* deprecated `TxOutSecrets::value_bf()`, replacement `TxOutSecrets::value_blinding_factor()`
