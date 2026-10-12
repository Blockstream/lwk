# Changelog

## Unreleased

* Update to `elements` 0.27 and `elements-miniscript` 0.5.
* Breaking: `get_genesis_hash` now returns `Option<BlockHash>` instead of `BlockHash` (all-zeros previously stood for "absent")
* Trait `Signer` has additional required method `network()`
