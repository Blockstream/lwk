# Changelog

## Unreleased

* Removed `Wollet::as_ref()`, replaced with `Wollet::ct_descriptor()`
* The following methods now return a `Result`:
  * `Wollet::descriptor()`
  * `WolletDescriptor::descriptor()`
  * `WolletDescriptor::url_encoded_descriptor()`
  * `WolletDescriptor::bitcoin_descriptor_without_key_origin()`
  * `WolletDescriptor::single_bitcoin_descriptors()`
* `Wollet::transaction()`, `Wollet::transactions()`, `Wollet::transactions_paginated()` populate inputs and outputs with explicit wallet inputs and outputs
* `Wollet::transactions()`, `Wollet::transactions_paginated()` also return transactions with explicit wallet inputs or outputs
* `Wollet::txos()` returns also explicit wallet txos
