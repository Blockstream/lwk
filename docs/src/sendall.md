# Send All Funds
Sending all funds (also called "draining" the wallet) allows you to send all LBTC (policy asset) from your wallet to a specified address in a single transaction. This is useful when you want to empty your wallet, consolidate UTXOs, or transfer all funds to another address.

When you drain a wallet, all available LBTC UTXOs are selected as inputs, and after deducting transaction fees, the remaining amount is sent to the destination address. Note that draining only affects LBTC (the policy asset).

"Drain" is only available for the asset used to pay fees (LBTC) with. For LBTC it's hard to guess the right amount to send all funds: it's the sum of all LBTC inputs minus the fee, but you don't know the actual fee until you created the transaction. So we need a way specify the TxBuilder to send all LBTC.

For assets that are not LBTC, the caller can easily compute the sum of all asset in input and set the satoshi value explicitly.

To send all LBTC, use `TxBuilder::drain_lbtc_wallet()`. By default LBTC are sent to a wallet address, if you want specify the destination address use `TxBuilder::drain_lbtc_to()`.

The methods take the following arguments:

1. **`drain_lbtc_wallet()`**: No arguments. Selects all available LBTC UTXOs from the wallet to be spent.
2. **`drain_lbtc_to(address)`**: Takes an `Address` parameter specifying where to send all the LBTC after fees are deducted.

<custom-tabs category="lang">
<div slot="title">Rust</div>
<section>

```rust,ignore
{{#include ../../lwk_wollet/tests/e2e.rs:drain_lbtc_wallet}}
```

</section>

<div slot="title">Python</div>
<section>

```python
{{#include ../../lwk_bindings/tests/bindings/drain_lbtc.py:drain_lbtc_wallet}}
```

</section>
</custom-tabs>

## Next Steps

After sending all funds, you can:
* Send other assets using regular [transaction creation](tx.md)
* Receive new funds at any address in your wallet

----

Previous: [Burn](burn.md)
