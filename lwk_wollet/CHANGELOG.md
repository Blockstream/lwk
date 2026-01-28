# Changelog

## Unreleased

* Removed `Wollet::as_ref()`, replaced with `Wollet::ct_descriptor()`
* The following methods now return a `Result`:
  * `Wollet::descriptor()`
  * `WolletDescriptor::descriptor()`
  * `WolletDescriptor::url_encoded_descriptor()`
  * `WolletDescriptor::bitcoin_descriptor_without_key_origin()`
  * `WolletDescriptor::single_bitcoin_descriptors()`
