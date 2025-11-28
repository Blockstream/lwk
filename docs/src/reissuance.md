# Reissuance
Asset reissuance on Liquid allows you to create additional units of an existing asset by spending a reissuance token. When you issue an asset, you can optionally create reissuance tokens that give the holder the right to create more of that asset in the future

The reissuance token is moved when you reissue the asset and you can reissue multiple times as long as you have reissuance tokens available.

To reissue an asset, use `TxBuilder::reissue_asset()` before calling `finish()`. The reissuance token must be owned by the wallet generating the reissuance transaction.

The `reissue_asset()` method takes the following arguments:

1. **`asset_to_reissue`** (`AssetId`): The ID of the asset you want to reissue. This is obtained from the original issuance transaction.
2. **`satoshi_to_reissue`** (`u64`): The number of new units (in satoshis) of the asset to create. Must be greater than 0 and cannot exceed 21,000,000 BTC (2,100,000,000,000,000 satoshis).
3. **`asset_receiver`** (`Option<Address>`): Optional address that will receive the newly reissued asset. If `None`, the asset will be sent to an address from the wallet generating the reissuance transaction.
4. **`issuance_tx`** (`Option<Transaction>`): Optional original issuance transaction. Required only if the wallet generating the reissuance didn't participate in the original issuance (i.e., the reissuance token was transferred to this wallet).

<custom-tabs category="lang">
<div slot="title">Rust</div>
<section>

```rust,ignore
{{#include ../../lwk_wollet/tests/e2e.rs:reissue_asset}}
```

</section>

<div slot="title">Python</div>
<section>

```python
{{#include ../../lwk_bindings/tests/bindings/issue_asset.py:reissue_asset}}
```

</section>
</custom-tabs>

## Next Steps

After reissuing an asset, you can:
* Reissue again if you have more reissuance tokens
* [Burn](burn.md) some of the asset to reduce the supply
* Send the asset to other addresses using regular [transaction creation](tx.md)

----

Previous: [Issuance](issuance.md)
