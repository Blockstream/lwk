# Python

## Install from PyPI

```shell
pip install lwk
```

## Build wheel

First, create a virtual env, skip the step if you already created it.

```shell
cd lwk/lwk_bindings
virtualenv venv
source venv/bin/activate
pip install maturin maturin[patchelf] uniffi-bindgen
```

Then build the wheel

```shell
cd lwk/lwk_bindings
maturin develop
```

Try it (note there is still an issue in how we import the package when using the wheel):

```python
import lwk
str(lwk.Network.mainnet())
```

## Examples

* [List transactions](https://github.com/Blockstream/lwk/blob/master/lwk_bindings/tests/bindings/list_transactions.py) of a wpkh/slip77 wallet, also compute the UTXO only balance
* [Send transaction](https://github.com/Blockstream/lwk/blob/master/lwk_bindings/tests/bindings/send_transaction.py) of a wpkh/slip77 wallet in a regtest environment
* [Send asset](https://github.com/Blockstream/lwk/blob/master/lwk_bindings/tests/bindings/send_asset.py) of a wpkh/slip77 wallet in a regtest environment
* [Issue a Liquid asset](https://github.com/Blockstream/lwk/blob/master/lwk_bindings/tests/bindings/issue_asset.py)
* AMP0 [setup](https://github.com/Blockstream/lwk/blob/master/lwk_bindings/tests/bindings/amp0-setup.py) and [daily operations](https://github.com/Blockstream/lwk/blob/master/lwk_bindings/tests/bindings/amp0-daily-ops.py) demonstrate Asset Management Platform version 0 integration
* [AMP2](https://github.com/Blockstream/lwk/blob/master/lwk_bindings/tests/bindings/amp2.py) demonstrates Asset Management Platform protocol integration
* [External unblinding](https://github.com/Blockstream/lwk/blob/master/lwk_bindings/tests/bindings/external_unblind.py) shows how to unblind transaction data externally
* [LiquiDEX](https://github.com/Blockstream/lwk/blob/master/lwk_bindings/tests/bindings/liquidex.py) demonstrates Liquid decentralized swap functionality
* [Manual coin selection](https://github.com/Blockstream/lwk/blob/master/lwk_bindings/tests/bindings/manual_coin_selection.py) shows how to manually select coins for transactions
* [Multisig](https://github.com/Blockstream/lwk/blob/master/lwk_bindings/tests/bindings/multisig.py) demonstrates multisignature wallet setup and usage
* [PSET details](https://github.com/Blockstream/lwk/blob/master/lwk_bindings/tests/bindings/pset_details.py) shows how to inspect and work with Partially Signed Elements Transactions
* [Authenticated Esplora](https://github.com/Blockstream/lwk/blob/master/lwk_bindings/tests/bindings/authenticated_esplora_client.py) connects to an authenticated Esplora/Waterfalls backend using OAuth2, a static token, or custom headers
* [Authenticated Electrum](https://github.com/Blockstream/lwk/blob/master/lwk_bindings/tests/bindings/authenticated_electrum_client.py) connects to an authenticated Electrum RPC proxy using OAuth2 or a static token
