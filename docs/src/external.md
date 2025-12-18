# Add External Inputs

External UTXOs allow you to include UTXOs from other wallets in your transaction. This is useful for collaborative transactions where multiple parties need to contribute inputs, such as in multi-party payments, atomic swaps, or when coordinating transactions between different wallets.

When you add external UTXOs to a transaction, the transaction builder will use them along with your wallet's UTXOs to cover the transaction amount and fees. The external wallet must sign the transaction separately, as these UTXOs are not owned by your wallet.

To use external UTXOs, you need to create an `ExternalUtxo` object with the necessary information from the external wallet, then call `TxBuilder::add_external_utxos()` before calling `finish()`. The external wallet must later add its signature details to the PSET and sign it.

The `add_external_utxos()` method takes the following argument:

1. **utxos** (`Vec<ExternalUtxo>`): A vector of external UTXO objects containing the outpoint, transaction output, unblinded values, and other metadata needed to spend the UTXO.

## Creating External UTXOs

Before adding external UTXOs to a transaction, you need to create `ExternalUtxo` objects. If you're working with another LWK wallet, you can extract the UTXO information from that wallet:

<custom-tabs category="lang">
<div slot="title">Rust</div>
<section>

```rust,ignore
{{#include ../../lwk_wollet/tests/e2e.rs:external_utxo_create}}
```

</section>

<div slot="title">Python</div>
<section>

```python
{{#include ../../lwk_bindings/tests/bindings/external_utxos.py:external_utxo_create}}
```

</section>
</custom-tabs>

## External UTXO for L-BTC

The simplest use case is including an external L-BTC UTXO in your transaction:

<custom-tabs category="lang">
<div slot="title">Rust</div>
<section>

```rust,ignore
{{#include ../../lwk_wollet/tests/e2e.rs:external_utxo_add}}
```

</section>

<div slot="title">Python</div>
<section>

```python
{{#include ../../lwk_bindings/tests/bindings/external_utxos.py:external_utxo_add}}
```

</section>
</custom-tabs>

## Signing with External Wallets

When you include external UTXOs in a transaction, the external wallet must add its signature details to the PSET and sign it. The external wallet uses `Wollet::add_details()` to add the necessary information, then signs the PSET:

<custom-tabs category="lang">
<div slot="title">Rust</div>
<section>

```rust,ignore
{{#include ../../lwk_wollet/tests/e2e.rs:external_utxo_sign}}
```

</section>

<div slot="title">Python</div>
<section>

```python
{{#include ../../lwk_bindings/tests/bindings/external_utxos.py:external_utxo_sign}}
```

</section>
</custom-tabs>

**Note**: The external wallet must call `add_details()` on the PSET before signing. This adds the necessary witness data and other information required for the external wallet to sign its inputs.

## Mixing Internal and External UTXOs

You can combine both your wallet's UTXOs and external UTXOs in the same transaction. 

<custom-tabs category="lang">
<div slot="title">Rust</div>
<section>

```rust,ignore
{{#include ../../lwk_wollet/tests/e2e.rs:external_utxo_mixed}}
```

</section>
</custom-tabs>

## Unblinded External UTXOs

You can also spend unblinded UTXOs (explicit asset/value) as external UTXOs. These are UTXOs that have explicit asset and value fields rather than being blinded. Use `Wollet::explicit_utxos()` to retrieve them:

<custom-tabs category="lang">
<div slot="title">Rust</div>
<section>

```rust,ignore
{{#include ../../lwk_wollet/tests/e2e.rs:external_utxo_unblinded}}
```

</section>
</custom-tabs>
