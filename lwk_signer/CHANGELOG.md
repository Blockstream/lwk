# Changelog

## Unreleased

* Add `SwSigner::from_seed_with_network()` to create a signer from a 64-byte BIP39 seed, so mnemonics in any BIP39 language and with a passphrase are supported; `seed()`, the SLIP77 master blinding key and BIP85 are available, `mnemonic()` returns `None`
* Add `NewError::InvalidSeedLength`
* Breaking: If a PSET has a non-default sighash, `SwSigner::sign()` and `sign_with_seckey()` now return an error. To sign with non-default sighashes, pass `SignOptions::allow_sighash_single_anyonecanpay()` or `SignOptions::dangerous_allow_any_sighash()` to `SwSigner::sign_with_options()` or `sign_with_seckey_with_options()`

## 0.20.0

* Update to `elements` 0.27.
* `SwSigner` now stores its `lwk_common::Network`
* Deprecate `SwSigner::new()`, add `SwSigner::new_with_network()`
* Deprecate `SwSigner::random()`, add `SwSigner::random_with_network()`
* Deprecate `SwSigner::from_xprv()`, add `SwSigner::from_xprv_with_network()`
