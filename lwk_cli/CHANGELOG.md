# Changelog

## Unreleased

* Change output of `lwk_cli wallet pset-details`: `fee : u64` replaced with `fees: {asset_id : u64}` ([ELIP-204](https://github.com/ElementsProject/ELIPs/blob/main/elip-0204.mediawiki))

## 0.13.0

* Rename `lwk_cli asset from-explorer` to `lwk_cli asset from-registry`
* Rename `lwk_cli wallet tx --from-explorer` to `lwk_cli wallet tx --fetch`
* Remove `--esplora-api-url` option from `lwk_cli server start`
* Rename `lwk_cli server start --electrum-url` to `lwk_cli server start --server-url`

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
