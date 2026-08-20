# Changelog

## Unreleased

* `SwSigner` now stores its `lwk_common::Network`. Add `SwSigner::network()`.
* Deprecate `SwSigner::new()`, add `SwSigner::new_with_network()`
* Deprecate `SwSigner::random()`, add `SwSigner::random_with_network()`
* Deprecate `SwSigner::from_xprv()`, add `SwSigner::from_xprv_with_network()`
