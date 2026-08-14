# Changelog

## Unreleased

* `asyncr::Jade::stream` returns `&Mutex<S>` instead of `&S`.
* PSET and message signatures produced through Jade's anti-exfil flow are now verified. PSET
  verification applies to ECDSA inputs signed through `sign`, not Taproot inputs or `sign_psbt`,
  because Jade does not yet support anti-exfil for those flows.
* PSET signing now honors requested ECDSA sighash types instead of always using `SIGHASH_ALL`.

## 0.18.0

* Changed `TxInputParams::is_witness` and `TxInputParams::path` to optional fields so unsigned Jade input placeholders can omit them without conflating omitted values with explicit `false` or root paths.
