# Transaction Creation
With a `Wollet` you can generate an address,
which can be used to receive some funds.
You can fetch the transactions receiving the funds using a "client",
and apply them to the wallet.
Now that the `Wollet` has a balance, it is able to craft transactions sending funds to desired destination.

The first step is constructing a `TxBuilder` (or `WolletTxBuilder`), using `TxBuilder::new()` or `Wollet::tx_builder()`.
You can now specify how to build the transaction using the methods exposed by the `TxBuilder`.

## Add a Recipient
If you want to send some funds you need this information:
* Address: destination (confidential) address provided by the receiver
* Amount: number of units of the asset (satoshi) to be sent.
* Asset: identifier of the asset that should be sent

Then you can call `TxBuilder::add_recipient()` to create an output which sends the amount of the asset, to the specified address.

You can add multiple recipients to the same transaction.

## Advanced Options
LWK allows to construct complex transactions, here there are few examples
* Set fee rate with `TxBuilder::fee_rate()`
* [Manual coin selection](manual.md)
* [External UTXOS](external.md)
* [Explicit inputs and outputs](explicit.md)
* [Send all LBTC](sendall.md)
* [Issuance](issuance.md), [reissuance](reissuance.md), [burn](burn.md)

## Construct the Transaction (PSET)
Once you set all the desired options to the `TxBuilder`.
You can construct the transaction calling `TxBuilder::finish()`.
This will return a Partially Signed Elements Transaction ([PSET](https://github.com/ElementsProject/elements/blob/master/doc/pset.mediawiki)),
a transaction encoded in a format that facilitates sharing the transaction with signers.

<custom-tabs category="lang">
<div slot="title">Rust</div>
<section>

```rust,ignore
{{#include ../../lwk_wollet/tests/e2e.rs:tx}}
```
</section>

<div slot="title">Python</div>
<section>

```python
{{#include ../../lwk_bindings/tests/bindings/basics.py:tx:ignore}}
```
</section>

<div slot="title">Javascript</div>
<section>

```typescript
{{#include ../../lwk_wasm/tests/node/basics.js:tx:ignore}}
```
</section>
</custom-tabs>

----

Next: [Sign Transaction](sign.md)
