# Issuance
Asset issuance on Liquid allows you to create new digital assets. When you issue an asset, you create a certain amount of that asset and optionally you also create another asset, called reissuance token, that allows you to create more of the asset later.

## Understanding Asset Issuance

When issuing an asset, you need to specify:
* **Asset amount**: The number of units (in satoshis) of the asset to create
* **Asset receiver**: The address that will receive the newly issued asset (optional, defaults to a wallet address)
* **Token amount**: The number of reissuance tokens to create (optional, can be 0)
* **Token receiver**: The address that will receive the reissuance tokens (optional, defaults to a wallet address)
* **Contract**: Metadata about the asset (optional, necessary for asset registry)

The asset receiver address and token receiver address can belong to different wallets with different spending policies, allowing for a more secure and customizable setup according to the issuer's needs.

The asset ID is deterministically derived from the transaction input and contract metadata (if provided). This means that if you use the same contract and transaction input, you'll get the same asset ID.

## Creating a Contract

A contract defines metadata about your asset, such as its name, ticker, precision, and issuer information. While contracts are optional, they are highly recommended as they allow your asset to be registered in the Liquid Asset Registry and displayed with proper metadata in wallets.

A contract contains:
* **domain**: The domain of the asset issuer (e.g., "example.com")
* **issuer_pubkey**: The public key of the issuer (33 bytes, hex-encoded)
* **name**: The name of the asset (1-255 ASCII characters)
* **precision**: Decimal precision (0-8, where 8 is like Bitcoin)
* **ticker**: The ticker symbol (3-24 characters, letters, numbers, dots, and hyphens)
* **version**: Contract version (currently only 0 is supported)

Amounts expressed in satoshi are always whole numbers without any decimal places. If you want to represent decimal values, you should use the "precision" variable in the contract. This variable determines the number of decimal places, i.e., how many digits appear after the decimal point. The following table shows different precision values with the issuance of 1 million satoshi.

|Satoshi  |Precision|Asset units|
|---------|---------|-----------|
|1.000.000|0        |1.000.000  |
|1.000.000|1        |100.000,0  |
|1.000.000|2        |10.000,00  |
|1.000.000|3        |1.000,000  |
|1.000.000|4        |100,000    |
|1.000.000|5        |10,00000   |
|1.000.000|6        |1,000000   |
|1.000.000|8        |0.1000000  |
|1.000.000|8        |0.01000000 |

<custom-tabs category="lang">
<div slot="title">Rust</div>
<section>

```rust,ignore
{{#include ../../lwk_wollet/tests/e2e.rs:contract}}
```

</section>

<div slot="title">Python</div>
<section>

```python
{{#include ../../lwk_bindings/tests/bindings/issue_asset.py:contract}}
```

</section>
</custom-tabs>

## Issuing an Asset

To issue an asset, use `TxBuilder::issue_asset()` before calling `finish()`. You need to have some LBTC in your wallet to pay for the transaction fees.

<custom-tabs category="lang">
<div slot="title">Rust</div>
<section>

```rust,ignore
{{#include ../../lwk_wollet/tests/e2e.rs:issue_asset}}
```

</section>

<div slot="title">Python</div>
<section>

```python
{{#include ../../lwk_bindings/tests/bindings/issue_asset.py:issue_asset}}
```

</section>
</custom-tabs>

## Getting Asset and Token IDs

After creating the issuance PSET, you can extract the asset ID and reissuance token ID from the transaction input:

<custom-tabs category="lang">
<div slot="title">Rust</div>
<section>

```rust,ignore
{{#include ../../lwk_wollet/tests/e2e.rs:issuance_ids}}
```

</section>

<div slot="title">Python</div>
<section>

```python
{{#include ../../lwk_bindings/tests/bindings/issue_asset.py:issuance_ids}}
```

</section>
</custom-tabs>

## Complete Example

Here's a complete example that issues an asset, signs it, and broadcasts it:

<custom-tabs category="lang">
<div slot="title">Rust</div>
<section>

```rust,ignore
{{#include ../../lwk_wollet/tests/e2e.rs:test_issue_asset}}
```

</section>

<div slot="title">Python</div>
<section>

```python
{{#include ../../lwk_bindings/tests/bindings/issue_asset.py:test_issue_asset}}
```

</section>
</custom-tabs>

## Important Notes

* **Asset amount limit**: The maximum asset amount is 21,000,000 BTC (2,100,000,000,000,000 satoshis)
* **At least one amount required**: Either `asset_sats` or `token_sats` must be greater than 0
* **Reissuance tokens**: If you want to be able to create more of the asset later, you must issue at least 1 reissuance token. The holder of the reissuance token can use it to [reissue](reissuance.md) more of the asset
* **Contract commitment**: If a contract is provided, its metadata is committed in the asset ID. This means the asset ID will be the same if you use the same contract and transaction input
* **Confidential issuance**: The issuance amounts can be confidential (blinded) or explicit. LWK handles this automatically

## Next Steps

After issuing an asset, you can:
* [Reissue](reissuance.md) more of the asset if you have reissuance tokens
* [Burn](burn.md) some of the asset to reduce the supply
* Send the asset to other addresses using regular [transaction creation](tx.md)

----

Next: [Reissuance](reissuance.md)
