# AMP0 in LWK

[Asset Management Platform](https://blockstream.com/amp/) version 0 (AMP0) is a service for issuers that allows to enforce specific rules on certain Liquid assets (AMP0 assets).

AMP0 is based on a legacy system and it does not fit the LWK model perfectly.
Despite that _LWK has partial support for AMP0_.

If you need full support for AMP0, use [GDK](https://github.com/blockstream/gdk).

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

## Warnings

AMP0 is based on a legacy system and it has some pitfalls.
We put some mechanism in order to make it harder to do the wrong thing, anyway callers should be careful when getting new addresses and syncing the wallet.

### Getting new addresses
AMP0 server only monitors addresses that have been returned by the server.
If you send funds to an address that was not returned by the server, the AMP0 server will not cosign transactions spending that inputs.
Which means that those funds are lost (!), since AMP0 accounts are 2of2.

To avoid getting in these dangerous situations, for AMP0 accounts, callers:
* must NOT get addresses from `Wollet::address()` or `WolletDescriptor::address()`
* must only get addresses from `Amp0::address()`

### Syncing the wallet
AMP0 accounts do not have the concept of `GAP_LIMIT` and they can have several unused address in a row.
The default scanning mechanism when it sees enough unused addresses in a row it stops.
So it can happen that some transactions are not returned, and the wallet balance could be incorrect.

To overcome this, when syncing AMP0 account wallets, callers must:
* get the last address index returned, using `Amp0::last_index()`
* and then use `BlockchainBackend::full_scan_to_index()` to sync up to that index

## Setup

TODO: setup with blockstream App (sreen)

Alternative you can setup the AMP account using `green_cli`.

## AMP0 daily operations

### Receive

### Monitor

### Send
