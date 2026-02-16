# Watch-Only Wallets
In LWK, the core functions of a wallet are split between two components for enhanced security: **Signers** manage private keys, while the **Wollet** handles everything else.

The term "Wollet" is not a typo; it stands for "Watch-Only wallet." A wollet provides view-only access, allowing you to generate addresses and see your balance without ever handling private keys. This design is crucial for security, as it keeps your private keys isolated.

A LWK wollet can perform the following operations:
* Generate addresses
* List transactions
* Get balance
* Create transactions (but not sign them)

## CT descriptors
A Wollet is defined by a [CT descriptor](https://github.com/ElementsProject/ELIPs/blob/main/elip-0150.mediawiki), which consists in a Bitcoin descriptor plus the descriptor blinding key.

In the previous section, we saw how to generate a single sig CT descriptor from a signer with `Signer::wpkh_slip77_descriptor()`, which returns something like:
```ignore
ct(slip77(...),elwpkh([ffffffff/84h/1h/0h]xpub...))
```
* `ct(...,...)`
* `slip77(...)` the descriptor blinding key
* `el` the "Elements" prefix
* `wpkh([ffffffff/84h/1h/0h]xpub...)` the "Bitcoin descriptor", with

The CT descriptors defines the wallet spending conditions. In this case it requires a single signature from a specific signer.

LWK supports more complex spending conditions, such as [multisig](multisig.md).

## Create a Wollet
From the CT descriptor, you need to generate a `WolletDescriptor`. Calling `WolletDescriptor::from_str()` will perform some basic validation of the descriptor, and fails if the descriptor is not supported by LWK.

Once you have a `WolletDescriptor` you can create a `Wollet` using either `Wollet::without_persist()` (keeps wallet data in memory) or `Wollet::with_fs_persist()` (stores wallet data on filesystem).

LWK also allows `Wollet`s to have a [custom persister](persister.md).

<custom-tabs category="lang">
<div slot="title">Rust</div>
<section>

```rust,ignore
{{#include ../../lwk_wollet/tests/e2e.rs:wollet}}
```
</section>

<div slot="title">Python</div>
<section>

```python
{{#include ../../lwk_bindings/tests/bindings/basics.py:wollet:ignore}}
```
</section>

<div slot="title">Javascript</div>
<section>

```typescript
{{#include ../../lwk_wasm/tests/node/basics.js:wollet:ignore}}
```
</section>
</custom-tabs>

## Generate Addresses
You can generate a wallet confidential address with `Wollet::address()`.

This address can receive any Liquid asset or amount.

<custom-tabs category="lang">
<div slot="title">Rust</div>
<section>

```rust,ignore
{{#include ../../lwk_wollet/tests/e2e.rs:address}}
```
</section>

<div slot="title">Python</div>
<section>

```python
{{#include ../../lwk_bindings/tests/bindings/basics.py:address:ignore}}
```
</section>

<div slot="title">Javascript</div>
<section>

```typescript
{{#include ../../lwk_wasm/tests/node/basics.js:address:ignore}}
```
</section>
</custom-tabs>

## Get Transactions and Balance
It's possibile to get the list of wallet transactions with `Wollet::transactions()` and the balance `Wollet::balance()`.

Note: Liquid transactions are confidential, meaning that only sender and receiver can see their asset and amount. `Wollet` unblinds the transactions and returns unblinded data that can be shown to the user.

`Wollet` however does not have internet access.
To fetch (new) wallet data, you need to use a "client" that fetches wallet transactions from some server.
In the next section we explain how (new) blockchain data can be obtained and added to the wallet.

<custom-tabs category="lang">
<div slot="title">Rust</div>
<section>

```rust,ignore
{{#include ../../lwk_wollet/tests/e2e.rs:txs}}
```
</section>

<div slot="title">Python</div>
<section>

```python
{{#include ../../lwk_bindings/tests/bindings/basics.py:txs:ignore}}
```
</section>

<div slot="title">Javascript</div>
<section>

```typescript
{{#include ../../lwk_wasm/tests/node/basics.js:txs:ignore}}
```
</section>
</custom-tabs>

----

Next: [Update the Wallet](scan.md)
