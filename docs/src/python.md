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

* [List transactions](../lwk_bindings/tests/bindings/list_transactions.py) of a wpkh/slip77 wallet, also compute the UTXO only balance
* [Send transaction](../lwk_bindings/tests/bindings/send_transaction.py) of a wpkh/slip77 wallet in a regtest environment
* [Send asset](../lwk_bindings/tests/bindings/send_asset.py) of a wpkh/slip77 wallet in a regtest environment
* [Issue a Liquid asset](../lwk_bindings/tests/bindings/issue_asset.py)
* [Custom persister](../lwk_bindings/tests/bindings/custom_persister.py), the caller code provide how the wallet updates are persisted
* [AMP0](../lwk_bindings/tests/bindings/amp0.py) demonstrates Asset Management Platform version 0 integration
* [AMP2](../lwk_bindings/tests/bindings/amp2.py) demonstrates Asset Management Platform protocol integration
* [External unblinding](../lwk_bindings/tests/bindings/external_unblind.py) shows how to unblind transaction data externally
* [LiquiDEX](../lwk_bindings/tests/bindings/liquidex.py) demonstrates Liquid decentralized swap functionality
* [Manual coin selection](../lwk_bindings/tests/bindings/manual_coin_selection.py) shows how to manually select coins for transactions
* [Multisig](../lwk_bindings/tests/bindings/multisig.py) demonstrates multisignature wallet setup and usage
* [PSET details](../lwk_bindings/tests/bindings/pset_details.py) shows how to inspect and work with Partially Signed Elements Transactions
