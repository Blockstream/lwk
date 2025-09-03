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
