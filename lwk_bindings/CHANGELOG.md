# Changelog

## Unreleased

## 0.19.0

* Added `DerivationPath`
* Removed `get_path()`, replaced with `DerivationPath::ss_path()`
* Changed "simplicity" functions `derive_keypair()`, `simplicity_derive_xonly_pubkey()`, `create_p2pk_signature()` to accept `DerivatioPath` instead of `str`
* Added `has_txs()` to `ElectrumClient`, `EsploraClient` and `WaterfallsClient`, to cheaply check if a descriptor has transaction history without a full scan
* Fixed Boltz chain-swap restoration to use the binding session network instead of Regtest
* Added `Signer::ss_desc()`
* Renamed `WolletDescriptor::from_xpub()` to `WolletDescriptor::ss_desc_from_external_signer()`; `account_type` is now restricted to "wpkh"/"shwpkh" (was also "pkh"/"tr"), and `master_blinding_key` must now be the raw SLIP77 key hex, not wrapped in "slip77(...)"

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
