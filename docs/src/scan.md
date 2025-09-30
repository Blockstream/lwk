# Update the Wallet
The fact that `Wollet` does have access to internet is a deliberate choice.
This allows `Wollet` to work offline, where they can generate addresses.

The connection is handled by a specific component, a Blockchain **Client**.
Blockchain clients connect to the specified server a fetch the wallet data from the blockchain.

LWK currently support 3 types of servers:
* Electrum Servers
* Esplora Servers
* Waterfalls Servers

To delve into their differences and strength points see our [dedicated section](clients.md).

## Create a Client
In this guide we will use an `EsploraClient`.

You can create a new client with `EsploraClient::new()`, specifying the URL of the service.

## Scan the Blockchain
Given a `Wollet` you can call `EsploraClient::full_scan()`,
which performs a series of network calls that scan the blockchain to find transactions relevant for the wallet.

`EsploraClient::full_scan()` has a stopping mechanisms that relies on BIP44 GAP LIMIT.
This might not fit every use cases.
In case you have large sequences of consecutive unused addresses you can use
`EsploraClient::full_scan_to_index()`.

## Apply the Update
`EsploraClient::full_scan()` fetches, parses, (locally) unblind and serialized the fetched data in returned value, an `Update`.
The `Update` can be applied to the `Wollet` using `Wollet::apply_update()`.

After applying the update the wollet data will include the new transaction downloaded,
for instance more transactions can be returned and balance can increase (or decrease).

<custom-tabs category="lang">
<div slot="title">Rust</div>
<section>

```rust,ignore
{{#include ../../lwk_wollet/tests/e2e.rs:client}}
```
</section>

<div slot="title">Python</div>
<section>

```python
{{#include ../../lwk_bindings/tests/bindings/basics.py:client:ignore}}
```
</section>

<div slot="title">Javascript</div>
<section>

```typescript
{{#include ../../lwk_wasm/tests/node/basics.js:client:ignore}}
```
</section>
</custom-tabs>

----

Next: [Create a transaction](tx.md)
