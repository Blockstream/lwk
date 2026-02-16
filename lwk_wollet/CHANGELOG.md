# Changelog

## Unreleased

There have been changes in how the wollet handle persistence.

* If you're a lwk_cli user, a lwk_wasm/lwk_node user, a lwk_bindings user using lwk_bindings::Wollet::new there are no change.

* if you're a lwk_bindings user using a custom persister:
  * removed `ForeignPersister` (trait), replacement `ForeignStore`
  * removed `ForeignPersisterLink` (concrete "trait"), replacement `ForeignStoreLink`
  * removed `Wollet::with_custom_persister()`, added `Wollet::with_custom_store()`

* if you're a lwk_wollet user:
  * removed `PersistError`
  * removed `Persister` (trait), replacement `lwk_common::Store` and `lwk_common::DynStore`
  * removed `NoPersist`, replacement `lwk_common::FakeStore`
  * removed `FsPersister`, replacement `lwk_common::FileStore` and `lwk_common::EncryptedStore`
  * removed `WolletBuilder::with_persister()`, replacement `WolletBuilder::with_store()`
  * changed `Wollet::new()`: 2nd argument is a `DynStore` instead of a "`Persister`"


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
