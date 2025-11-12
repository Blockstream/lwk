# Changelog

## Unreleased

* Rename `cli asset from-explorer` to `lwk_cli asset from-registry`

## 0.5.1

Add wallet drain (send all) support for L-BTC.

## 0.5.0

Fix ELIP-151 computation, note that ELIP151 wallets will generate
different addresses.

## 0.4.0

At startup if you had existing signers, wallets or assets,
you might incur in some errors.
To upgrade the state, if the error involves:
* `"asset_insert"`, get the contract, remove the line in `state.lock`
  and insert again asset from cli
* `"load_wallet"`, replace with `"wallet_load"`
* `"signer_load_software"`, add `,"persist":true`
