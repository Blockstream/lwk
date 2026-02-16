# AMP0 in LWK

[AMP0](https://blockstream.com/amp/) (Asset Management Platform version 0) is a service for issuers that allows to enforce specific rules on certain Liquid assets (AMP0 assets).

AMP0 is based on a legacy system and it does not fit the LWK model perfectly.
That is reflected in the LWK AMP0 interface which could be a bit cumbersome to use.

## Limitations
_LWK has partial support for AMP0._
For instance it does not allow to issue AMP0 asset, or use accounts with 2FA.

|                                 | LWK | GDK | AMP0 API |
|---------------------------------| :-: | :-: | :------: |
|Create AMP0 accounts             | ✅ | ✅ | ❌ |
|Receive on AMP0 accounts         | ✅ | ✅ | ❌ |
|Monitor AMP0 accounts            | ✅ | ✅ | ❌ |
|Send from AMP0 accounts          | ✅ | ✅ | ❌ |
|Account with 2FA                 | ❌ | ✅ | ❌ |
|issue, reissue, burn AMP0 assets | ❌ | ❌ | ✅ |
|set restriction for AMP0 assets  | ❌ | ❌ | ✅ |

If you need full support for AMP0, use [GDK](https://github.com/blockstream/gdk) and the AMP0 issuer API.

## Overview

To use AMP0 with LWK you need:
* 👀 some Green Watch-Only credentials (username and password) for a Green Wallet with an AMP account
* 🔑 the corresponding signer available (e.g. Jade or software with the BIP39 mnemonic)

Then you can:
* get addresses for the AMP0 account (👀)
* monitor the AMP0 account (get balance and transactions) (👀)
* create AMP0 transactions (👀)
* sign AMP0 transactions (🔑)
* ask AMP0 to cosign transactions (👀)
* broadcast AMP0 transactions (👀)

Using AMP0 with LWK you can keep the signer separated and operate it according to the desired degree of security and isolation.

<div class="warning">
⚠️ AMP0 is based on a legacy system and it has some pitfalls.
We put some mechanism in order to make it harder to do the wrong thing, anyway callers should be careful when getting new addresses and syncing the wallet.
</div>

## Setup
To use AMP0 with LWK you need to:
1. Create a Liquid wallet (backup its mnemonic/seed)
2. Create an AMP account (AMP ID)
3. Create a Liquid Watch-Only (username and password)

### 1. Create Liquid wallet
Create a `Signer` and backup its mnemonic/seed.
From the signer get its `signer_data` using `Signer::amp0_signer_data()`.

Create a `Amp0Connected::new()` passing the `signer_data`.
You now need to authenticate with AMP0 server.
First get the server challenge with `Amp0Connected::get_challenge()`.
Sign the challenge with `Signer::amp0_sign_challenge()`.
You can now call `Amp0Connected::login()` passing the signature.
This function returns a `Amp0LoggedIn` instance, which can be used to create new AMP0 accounts and watch-only entries.

### 2. Create an AMP account
Obtain the number of the next account using `Amp0LoggedIn::next_account()`.
Use the signer to get the corresponding xpub `Signer::amp0_account_xpub()`.
Now you can create a new AMP0 account with `Amp0LoggedIn::create_amp0_account()`, which returns the AMP ID.

### 3. Create a Liquid Watch-Only
Choose your AMP0 Watch-Only credentials `username` and `password` and call `Amp0LoggedIn::create_watch_only()`.


Now that you have mnemonic/seed (or Jade), AMP ID and Watch-Only credentials (username and password), you're ready to use AMP0 with LWK.

> If you're using `lwk_node`, polyfill the websocket
> ```typescript
> const WebSocket = require('ws');
> global.WebSocket = WebSocket;
> const lwk = require('lwk_node');
> ```

<custom-tabs category="lang">
<div slot="title">Rust</div>
<section>

```rust,ignore
{{#include ../../lwk_wollet/tests/e2e.rs:amp0-setup:ignore}}
```
</section>

<div slot="title">Python</div>
<section>

```python
{{#include ../../lwk_bindings/tests/bindings/amp0-setup.py:amp0-setup:ignore}}
```
</section>

<div slot="title">Javascript</div>
<section>

```typescript
{{#include ../../lwk_wasm/tests/node/amp0-setup.js:amp0-setup:ignore}}
```
</section>
</custom-tabs>

### Alternative setup
It's possible to setup an AMP0 account using GDK based apps:
1. [Blockstream App](https://blockstream.com/app/) (easiest, GUI, mobile, desktop, Jade support), or
2. [`green_cli`](https://github.com/Blockstream/green_cli/) (CLI, Jade support), or
3. [GDK](https://github.com/blockstream/gdk) directly (fastest, [example](gdk-amp0.py))

## AMP0 daily operations

LWK allows to manage created AMP0 accounts.
You can receive funds, monitor transactions and send to other wallets.

### Receive
To receive funds you need an address, you can get addresses with `Amp0::address()`.

<div class="warning">
⚠️ For AMP0 wallets, do not use <code>Wollet::address()</code> or <code>WolletDescriptor::address()</code>, using them can lead to loss of funds.
AMP0 server only monitors addresses that have been returned by the server.
If you send funds to an address that was not returned by the server, the AMP0 server will not cosign transactions spending that inputs.
Which means that those funds are lost (!), since AMP0 accounts are 2of2.
</div>

### Monitor
LWK allows to monitor Liquid wallets, including AMP0 accounts.

First you get the AMP0 descriptor with `Amp0::wollet_descriptor()`.
You then create a wallet with `Wollet::new()`.

Once you have the AMP0 `Wollet`, you can get `Wollet::transactions()`, `Wollet::balance()` and other information.

LWK wallets needs to be updated with new data from the Liquid blockchain.
First create a blockchain client, for instance `EsploraClient::new()`.
Then get an update with `BlockchainBackend::full_scan_to_index()` passing the value returned by `Amp0::last_index()`.
Finally update the wallet with `Wollet::apply_update()`.

<div class="warning">
⚠️ For AMP0 wallets, do not sync the wallet with <code>BlockchainBackend::full_scan()</code>, otherwise some funds might not show up.
AMP0 accounts do not have the concept of <code>GAP_LIMIT</code> and they can have several unused addresses in a row.
The default scanning mechanism when it sees enough unused addresses in a row it stops.
So it can happen that some transactions are not returned, and the wallet balance could be incorrect.
</div>

### Send
For AMP0 you can follow the standard LWK transaction flow, with few small differences.

Use the `TxBuilder`, add recipients `TxBuilder::add_recipient()`, and use the other available methods if needed.

Then instead of using `TxBuilder::finish()`, use `TxBuilder::finish_for_amp0()`.
This creates an `Amp0Pset` which contains the PSET and the `blinding_nonces`, some extra data needed by the AMP0 cosigner.

Now you need to interact with secret key material (🔑) corresponding to this AMP0 account.
Create a signer, using `SWSigner` or `Jade` and sign the PSET with the signer, using `Signer::sign()`.

Once the PSET is signed, you need to have it cosigned by AMP0.
Construct an `Amp0Pset` using the signed PSET and the `blinding_nonces` obtained before.
Call `Amp0::sign()` passing the signed `Amp0Pset`.

If all the AMP0 rules are respected, the transaction is cosigned by AMP0 and can be broadcast, e.g. with `EsploraClient::broadcast()`.

<custom-tabs category="lang">
<div slot="title">Rust</div>
<section>

```rust,ignore
{{#include ../../lwk_wollet/tests/e2e.rs:amp0-daily-ops:ignore}}
```
</section>

<div slot="title">Python</div>
<section>

```python
{{#include ../../lwk_bindings/tests/bindings/amp0-daily-ops.py:amp0-daily-ops:ignore}}
```
</section>

<div slot="title">Javascript</div>
<section>

```typescript
{{#include ../../lwk_wasm/tests/node/amp0-daily-ops.js:amp0-daily-ops:ignore}}
```
</section>
</custom-tabs>

## Examples
We provide a few examples on how to use AMP0 with LWK:
* [liquidwebwallet.org](https://liquidwebwallet.org) integrates AMP0 using WASM
* Rust tests in [amp0.rs](../lwk_wollet/src/amp0.rs)
