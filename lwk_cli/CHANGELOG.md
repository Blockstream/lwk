# Changelog

## Unreleased

## 0.19.0

* Breaking: `server start` now writes a random RPC auth cookie to `<datadir>/<network>/.cookie`
  and the server rejects requests without a matching `Authorization` header.
  `lwk_cli` reads the cookie automatically; a raw `curl` (or other direct HTTP client) call now
  needs `-u "$(cat <datadir>/<network>/.cookie)"`. `--datadir` moved from `server start` to a
  top-level option, since every command now needs it to find the cookie file, not just `server start`.

## 0.18.0

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
