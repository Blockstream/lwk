# Burn
Asset burning on Liquid allows you to provably destroy units of an asset, reducing the total supply. This is useful for various purposes such as token buybacks, reducing inflation, or implementing deflationary mechanisms.

When you burn an asset, the units are permanently removed from circulation and cannot be recovered. The burn operation creates a special output in the transaction that destroys the specified amount of the asset.

To burn an asset, use `TxBuilder::add_burn()` before calling `finish()`. The asset must be owned by the wallet generating the burn transaction.

The `add_burn()` method takes the following arguments:

1. **`satoshi`** (`u64`): The number of units (in satoshis) of the asset to burn. Must be greater than 0.
2. **`asset`** (`AssetId`): The ID of the asset you want to burn. This is obtained from the original issuance transaction or from your wallet's balance.

<custom-tabs category="lang">
<div slot="title">Rust</div>
<section>

```rust,ignore
{{#include ../../lwk_wollet/tests/e2e.rs:burn_asset}}
```

</section>

<div slot="title">Python</div>
<section>

```python
{{#include ../../lwk_bindings/tests/bindings/issue_asset.py:burn_asset}}
```

</section>
</custom-tabs>

## Next Steps

After burning an asset, you can:
* [Reissue](reissuance.md) the asset if you have reissuance tokens available
* Send the remaining asset to other addresses using regular [transaction creation](tx.md)

----

Previous: [Reissuance](reissuance.md)
