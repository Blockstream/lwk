# Changelog

## Unreleased

## 0.18.0

* Changed `TxInputParams::is_witness` and `TxInputParams::path` to optional fields so unsigned Jade input placeholders can omit them without conflating omitted values with explicit `false` or root paths.
