# Manual Coin Selection
Manual coin selection allows you to explicitly choose which UTXOs (unspent transaction outputs) from your wallet to use when building a transaction. By default, LWK automatically add all UTXOs to cover the transaction amount and fees. However, manual coin selection gives you control over which specific UTXOs are spent, which can be useful for privacy, UTXO management, or specific transaction requirements.

When you enable manual coin selection, only the UTXOs you specify will be used. The transaction builder will not automatically add additional UTXOs, so you must ensure the selected UTXOs provide sufficient funds to cover the transaction amount and fees.

To use manual coin selection, first retrieve the available UTXOs from your wallet using `Wollet::utxos()`, then call `TxBuilder::set_wallet_utxos()` before calling `finish()`. The selected UTXOs must belong to the wallet generating the transaction.

The `set_wallet_utxos()` method takes the following argument:

1. **`utxos`** (`Vec<OutPoint>`): A vector of outpoints (transaction ID and output index) identifying the UTXOs to use. Each outpoint must correspond to a UTXO owned by the wallet.

## Getting Available UTXOs

Before selecting UTXOs manually, you need to retrieve the list of available UTXOs from your wallet:

<custom-tabs category="lang">
<div slot="title">Rust</div>
<section>

```rust,ignore
{{#include ../../lwk_wollet/tests/e2e.rs:get_utxos}}
```

</section>

<div slot="title">Python</div>
<section>

```python
{{#include ../../lwk_bindings/tests/bindings/manual_coin_selection.py:get_utxos}}
```

</section>
</custom-tabs>

## Manual Coin Selection for L-BTC

The simplest use case is selecting specific L-BTC UTXOs for a transaction:

<custom-tabs category="lang">
<div slot="title">Rust</div>
<section>

```rust,ignore
{{#include ../../lwk_wollet/tests/e2e.rs:manual_coin_selection}}
```

</section>

<div slot="title">Python</div>
<section>

```python
{{#include ../../lwk_bindings/tests/bindings/manual_coin_selection.py:manual_coin_selection}}
```

</section>
</custom-tabs>

## Manual Coin Selection with Assets

When sending assets, you must include sufficient L-BTC UTXOs to cover transaction fees. If you only select asset UTXOs without L-BTC, the transaction will fail with an `InsufficientFunds` error.

## Next Steps

After learning about manual coin selection, you can:
* Learn about [external UTXOs](external.md) for using UTXOs not in your wallet
* Explore [transaction creation](tx.md) for other transaction building options
* Use [send all funds](sendall.md) to automatically select all L-BTC UTXOs

----

Previous: [Transaction Creation](tx.md)
