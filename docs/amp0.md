# AMP0 in LWK

[AMP0](https://blockstream.com/amp/) (Asset Management Platform version 0) is a service for issuers that allows to enforce specific rules on certain Liquid assets (AMP0 assets).

AMP0 is based on a legacy system and it does not fit the LWK model perfectly.
Despite that _LWK has partial support for AMP0_.

LWK allows to:
* Receive on AMP0 accounts
* Monitor AMP0 accounts
* Send from AMP0 accounts

LWK does NOT allow to:
* create AMP0 accounts
* issue, reissue and burn AMP0 assets
* interact with the AMP0 issuer interface

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

Using AMP0 with LWK you can keep the signer separated and operate it accoriding to the desired degree of security and isolation.

> [!WARNING]
> ⚠️ AMP0 is based on a legacy system and it has some pitfalls.
> We put some mechanism in order to make it harder to do the wrong thing, anyway callers should be careful when getting new addresses and syncing the wallet.

## Setup

To complete the setup procedure you can use:
* [Blockstream App](https://blockstream.com/app/), or
* [`green_cli`](https://github.com/Blockstream/green_cli/), or
* [GDK](https://github.com/blockstream/gdk) directly (e.g. with the released python wheels)

You need to:
1. Create a Liquid wallet (backup its mnemonic/seed)
2. Create a AMP account (AMP ID)
3. Create a Liquid Watch-Only (username and password)

Note that you can create the Liquid wallet using a Jade.

Now that you have:
* Mnemonic/seed (or Jade)
* AMP ID
* Watch-Only credentials (username and password)

You're ready to use AMP0 with LWK.

## AMP0 daily operations

LWK allows to manage created AMP0 accounts.
You can receive funds, monitor transactions and send to other wallets.

### Receive
To receive funds you need an address, you can get addresses with `Amp0::address()`.

> [!WARNING]
> ⚠️ For AMP0 wallets, do not use `Wollet::address()` or `WolletDescriptor::address()`, using them can lead to loss of funds.
> AMP0 server only monitors addresses that have been returned by the server.
> If you send funds to an address that was not returned by the server, the AMP0 server will not cosign transactions spending that inputs.
> Which means that those funds are lost (!), since AMP0 accounts are 2of2.

### Monitor
LWK allows to monitor Liquid wallets, including AMP0 accounts.

First you get the AMP0 descriptor with `Amp0::wollet_descriptor()`.
You then create a wallet with `Wollet::new()`.

Once you have the AMP0 `Wollet`, you can get `Wollet::transactions()`, `Wollet::balance()` and other information.

LWK wallets needs to be updated with new data from the Liquid blockchain.
First create a blockchain client, for insance `EsploraClient::new`.
Then get an update with `BlockchainBackend::full_scan_to_index()` passing the value returned by Amp0::last_index()`.
Finally update the wallet with `Wollet::apply_update()`.

> [!WARNING]
> ⚠️ For AMP0 wallets, do not sync the wallet with `BlockchainBackend::full_scan()`, otherwise some funds might not show up.
> AMP0 accounts do not have the concept of `GAP_LIMIT` and they can have several unused address in a row.
> The default scanning mechanism when it sees enough unused addresses in a row it stops.
> So it can happen that some transactions are not returned, and the wallet balance could be incorrect.

### Send
For AMP0 you can follow the standard LWK transaction flow, with few small differences.

Use the `TxBuilder`, add recipients `TxBuilder::add_recipient`, and use the other available methods if needed.

Then instead of using `TxBuilder::finish`, use `TxBuilder::finish_for_amp0`.
This creates an `Amp0Pset` which contains the PSET and the `blinding_nonces`, some extra data needed by the AMP0 cosigner.

Now you need to interact with secret key material (🔑) corresponding to this AMP0 account.
Create a signer, using `SWSigner` or `Jade` and sign the PSET with the signer, using `Signer::sign`.

Once the PSET is signed, you need to have it cosigned by AMP0.
Construct an `Amp0Pset` using the signed PSET and the `blinding_nonces` obtained before.
Call `Amp0::sign` passing the signed `Amp0Pset`.

If all the AMP0 rules are respected, the transaction is cosigned by AMP0 and can be broadcast, e.g. with `EsploraClient::broadcast()`.
