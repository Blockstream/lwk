# Changelog

## Unreleased

* Breaking: `get_genesis_hash` now returns `Option<BlockHash>` instead of `BlockHash` (all-zeros previously stood for "absent")
* Trait `Signer` has additional required method `network()`
