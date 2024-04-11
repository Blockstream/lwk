# Changelog

## Unreleased

At startup if you had existing signers, wallets or assets,
you might incur in some errors.
To upgrade the state, if the error involves:
* `"asset_insert"`, get the contract, remove the line in `state.lock`
  and insert again asset from cli
* `"load_wallet"`, replace with `"wallet_load"`
* `"signer_load_software"`, add `,"persist":true`
