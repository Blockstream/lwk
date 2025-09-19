# Users of LWK
This section showcases a series of projects that are built on LWK or its components.

| Name            | Description           | Type   | Language |
|-----------------|-----------------------|--------|----------|
|[Liquidtestnet.com](https://liquidtestnet.com)| Testnet Faucet| Server| Python|
|[Liquidwebwallet.org](https://liquidwebwallet.org)| Browser wallet| Wallet| Wasm|
|[AMP2](https://amp2.testnet.blockstream.com/info/spec)| Registered Assets| Server| Closed source|
|[Breeze SDK Liquid](https://github.com/breez/breez-sdk-liquid)| Lightning swaps| SDK| Rust|
|[Boltz](https://boltz.exchange/)| Atomic swaps| Server| Go|
|[Aqua](https://aqua.net/)| Atomic swaps| Wallet| Dart|
|[Bull Bitcoin](https://wallet.bullbitcoin.com/)| Atomic swaps| Wallet| Dart|
|[Blitz Wallet](https://blitz-wallet.com/)| Atomic swaps| Wallet| Javascript|
|[Peerswap](https://www.peerswap.dev/)| LN Balancing| Server| Go|
|[AreaLayer FireBolt wallet](https://www.arealayer.net/projects#h.kk9ofahkprw7)| Wallet| Wallet| TypeScript|
|[Banco Libre](https://bancolibre.com/)| Browser wallet| Wallet| Wasm|
|[Misty Breeze](https://breez.technology/)| Lightning wallet| Wallet| Dart|
|[Onion Mill - StashPay](https://blog.onionmill.com/)| Lightning wallet| Wallet| |
|[PSET GUI](https://github.com/dev4bitcoin/psetgui/)| PSET Analyzer| Tool| JavaScript|
|[Satsails](https://www.satsails.com/)| Wallet| Wallet| Dart|
|[Shopstr](https://shopstr.store/)| Nostr Marketplace| Server| TypeScript|


Feel free to open a PR to add or remove your product.

## Liquidtestnet.com
[Liquidtestnet.com](https://liquidtestnet.com) provides a several utilities to interact with Liquid testnet.
It also has a testnet faucet which distributes testnet assets (LBTC, standard assets and AMP0, AMP2 assets).

The faucet is built using LWK python wheels.

Liquidtestnet.com is open source, source available in the [Github repository](https://github.com/valerio-vaccaro/liquidtestnet.com).


## Liquidwebwallet.org
[Liquidwebwallet.org](https://liquidwebwallet.org) is a companion app for Liquid running in the browser. 
The website allows the use of read-only wallets or hardware wallets such as Jade or Ledger to access one's wallet, view the balance, create transactions, create issuance, reissuance, and burn transactions.

The wallet is built using [LWK_wasm](https://www.npmjs.com/package/lwk_wasm).

Liquidwebwallet.org is open source, source available in the [Github repository](https://github.com/RCasatta/liquid-web-wallet)


## AMP2
[AMP2](https://amp2.testnet.blockstream.com/info/spec) is a platform able to issue and manage digital assets on the Liquid Network with flexible API.
The platform allows for the management of the entire token lifecycle, enabling the control and authorization of each individual operation.

AMP2 uses LWK internally.

AMP2 is closed source.


## Breeze SDK Liquid 
The [Breeze SDK Liquid](https://github.com/breez/breez-sdk-liquid) provides developers with a end-to-end solution for integrating self-custodial Lightning payments into their apps and services. 

The SDK use LWK as internal liquid wallet and signer.

Breeze SDK Liquid is open source, source available in the [Github repository](https://github.com/breez/breez-sdk-liquid)


## Boltz
[Boltz](https://boltz.exchange/) the Non-Custodial Bitcoin Bridge able to swap between different Bitcoin layers.

The SDK use LWK as internal liquid wallet and signer.

Boltz is open source, source available in the [Github repository](https://github.com/BoltzExchange/boltz-client)


## Aqua
[Aqua](https://aqua.net/) AQUA is a free, open-source wallet for iOS and Android.

The app use LWK in the backend.

Aqua is open source, source available in the [Github repository](https://github.com/AquaWallet/aqua-wallet)


## Bull Bitcoin
[Bull Bitcoin](https://wallet.bullbitcoin.com/) is a mobile wallet with Bitcoin, Liquid and lightning support.

Bull Bitcoin uses LWK for its Liquid wallet.

Bull Bitcoin is open source, source available in the [Github repository](https://github.com/SatoshiPortal/bullbitcoin-mobile)


## Blitz Wallet
[Blitz Wallet](https://blitz-wallet.com/) is a react native wallet.

The app is based on Breeze SDK Liquid.

Blitz Wallet is open source, source available in the [Github repository](https://github.com/BlitzWallet/BlitzWallet)


## Peerswap
[Peerswap](https://www.peerswap.dev/) Atomic swaps for rebalancing Lightning channels.

The app use LWK as internal liquid wallet and signer.

Peerswap is open source, source available in the [Github repository](https://github.com/ElementsProject/peerswap)


## AreaLayer FireBolt wallet
|[AreaLayer FireBolt wallet](https://www.arealayer.net/projects#h.kk9ofahkprw7) is a react native wallet.

The app is based on Breeze SDK Liquid.

AreaLayer FireBolt wallet is open source, source available in the [Github repository](https://github.com/AreaLayer/firebolt-react-native)


## Banco Libre
[Banco Libre](https://bancolibre.com/) is a web wallet using LWK_wasm.

The app use LWK_wasm as internal liquid wallet and signer.

Banco Libre is open source, source available in the [Github repository](https://github.com/kipu-org/banco-client)


## Misty Breeze
[Misty Breeze](https://breez.technology/) is a flutter app based on Breez SDK Liquid.

The app is based on Breeze SDK Liquid.

Misty Breeze is open source, source available in the [Github repository](https://github.com/breez/misty-breez)


## Onion Mill - StashPay
[Onion Mill - StashPay](https://blog.onionmill.com/) is a  minimalist Bitcoin wallet based on Breeze SDK Liquid.

The app is based on Breeze SDK Liquid.

Onion Mill - StashPay is open source, source available in the [Github repository](https://github.com/onionmill/stashpay-bin)


## PSET GUI
[PSET GUI](https://github.com/dev4bitcoin/psetgui/) is a user-friendly application designed for analyzing and signing PSETs.

The app use LWK_wasm as internal liquid wallet and signer.

PSET GUI is open source, source available in the [Github repository](https://github.com/dev4bitcoin/psetgui/)


## Satsails
[Satsails](https://www.satsails.com/) is a self-custodial Bitcoin and Liquid wallet with support for stablecoins.

The app use LWK_wasm as internal liquid wallet and signer.

Satsails is open source, source available in the [Github repository](https://github.com/Satsails/Satsails)


## Shopstr
[Shopstr](https://shopstr.store/) is a global, permissionless Nostr marketplace for Bitcoin commerce.

WIP for [Liquid integration](https://github.com/shopstr-eng/shopstr/issues/74)

Shopstr is open source, source available in the [Github repository](https://github.com/shopstr-eng/)


# Unknown users
There may be many other users of the libraries who are currently unknown or not publicly disclosed

The libraries' license allows their integration into both open-source and closed-source solutions.These users can leverage lwk to:

- Develop wallets and online services for asset management,
- Create swap services,
- Issue, reissue, and burn assets.
