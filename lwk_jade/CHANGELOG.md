# Changelog

## Unreleased

* `asyncr::Jade::stream` returns `&Mutex<S>` instead of `&S`.
* PSET signatures produced through Jade's anti-exfil flow are now verified before being added.
* PSET signing now honors requested ECDSA sighash types instead of always using `SIGHASH_ALL`.
* Jade message signing now uses and verifies the anti-exfil protocol.

## 0.18.0

* Changed `TxInputParams::is_witness` and `TxInputParams::path` to optional fields so unsigned Jade input placeholders can omit them without conflating omitted values with explicit `false` or root paths.
