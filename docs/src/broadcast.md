# Transaction Broadcast
When a PSET has enough signatures, it's ready to be broadcasted to the Liquid Network.

## Finalize the PSET
First you need to call `Wollet::finalize()` to finalize the PSET and extract the signed transaction.

## Broadcast the Transaction
The transaction returned by the previous step can be sent to the Liquid Network with `EsploraClient::broadcast()`.

## Apply the Transaction
If you send a transaction you might want to see the balance decrease immediately.
With LWK this does not happens automatically,
you can do a "full scan" and apply the returned update.
However this requires network calls and it might be slow,
if you want your balance to be updated immediately,
you can call `Wollet::apply_tx()`.

<custom-tabs category="lang">
<div slot="title">Rust</div>
<section>

```rust,ignore
{{#include ../../lwk_wollet/tests/e2e.rs:broadcast}}
```
</section>

<div slot="title">Python</div>
<section>

```python
{{#include ../../lwk_wasm/tests/node/basics.js:broadcast:ignore}}
```
</section>

<div slot="title">Javascript</div>
<section>

```typescript
{{#include ../../lwk_wasm/tests/node/basics.js:broadcast:ignore}}
```
</section>
</custom-tabs>

----

Next: [Advanced Features](advanced.md)
