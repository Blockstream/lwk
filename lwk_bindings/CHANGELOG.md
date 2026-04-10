# Changelog

## Unreleased

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
