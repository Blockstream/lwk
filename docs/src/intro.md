# About LWK

The Liquid Wallet Kit (LWK) is a comprehensive toolkit that empowers developers to build a new generation of wallets and applications for the Liquid Network. Instead of grappling with the intricate, low-level details of Liquid's confidential transactions, asset management, and cryptographic primitives, LWK provides a powerful set of foundational building blocks. These tools are functional and secure, helping you build your projects with confidence.

LWK's primary goal is to abstract away complexity by handling the most challenging aspects of Liquid development, such as:
* **Confidential Transactions** handling, which automatically obscures amounts and asset types to maintain user privacy.
* **Asset issuance and management**, providing a seamless way to create and interact with new digital assets.
* **Signing Liquid transactions**, allowing for interaction with software signers and hardware wallets.

By providing these building blocks, LWK liberates developers from building Liquid functionality from scratch. This allows them to significantly accelerate development time and focus on creating unique, value-added features for their specific use cases, whether it's building a mobile wallet, integrating Liquid in an exchange, or developing a DeFi application. Ultimately, LWK is the definitive, go-to library for anyone committed to innovating on the Liquid Network.

## Example: single-sig mobile wallet

This example application showcases how the Liquid Wallet Kit (LWK) simplifies the development of a single-signature mobile wallet. The two diagrams below illustrate the key user flows: Wallet Creation and Transaction Management. LWK handles the complex, low-level interactions with the Liquid blockchain and cryptographic operations, allowing the application to focus on the user interface and experience.

### Wallet Creation

The mobile app starts by creating a new software `signer` and helps the user back up the corresponding BIP39 mnemonic. From this signer, the app extracts the `xpub` to derive a single-signature [CT descriptor](https://github.com/ElementsProject/ELIPs/blob/main/elip-0150.mediawiki) (e.g., `ct(slip77(...),elwpkh([...]xpub/<0;1>/*))`).

This CT descriptor is then used to initialize a `wollet`, which is LWK's watch-only wallet. The `wollet` allows the app to fetch addresses, transactions, and the current balance to display in the user interface.

When the app is opened, it uses a `client` to sync the wollet with the latest blockchain information. This ensures the wallet data is up-to-date.

```mermaid
flowchart TD
    Signer(Signer 🔑)
    Wollet("Wollet 👀<br>(descriptor)")
    Client(Client 🌐)
    App((📱))
    Signer -->|Xpub| Wollet 
    Client -->|Blockchain Update| Wollet
    Wollet -->|Addresses, Txs, Balance| App
```

### Transaction Management

The mobile app enables users to send funds by allowing them to specify the amount, asset, and destination address. The `wollet` then takes this information to create an unsigned transaction, which is encoded in the [PSET](https://github.com/ElementsProject/ELIPs/blob/main/elip-0150.mediawiki) format.

The PSET is passed to the `signer`, which uses its private keys to sign the transaction. Once the PSET is signed, it's finalized into a complete transaction, which the `client` then broadcasts to the Liquid Network.

```mermaid
flowchart TD
    Signer(Signer 🔑)
    Wollet("Wollet 👀<br>(descriptor)")
    Client(Client 🌐)
    App((📱))
    App -->|Create TX| Wollet
    Wollet -->|Unsigned PSET| Signer 
    Signer -->|Signed PSET| Wollet 
    Wollet -->|Broadcast TX| Client
```

### Remarks
This simple example highlights the core responsibilities of each LWK component:
* **Signer** 🔑: Manages private keys and handles all signing operations.
* **Wollet** 👀: Provides a watch-only view of the wallet, deriving addresses and tracking transactions and balances without holding any private keys.
* **Client** 🌐: Fetch blockchain data from the Liquid Network to update the `wollet`.

## Key Features
LWK allows to build more complex applications and products by leveraging its wide range of features:
* [x] Send and receive LBTC
* [x] Send and receive Liquid Issued Assets (e.g. USDT)
* [x] Send and receive AMP assets (e.g. BMN)
* [x] Software signers
* [x] Hardware wallets support (Jade)
* [x] Watch-Only view with CT descriptors
* [x] Single-sig
* [x] Generic Multisig
* [x] Multi-language support (Swift, Kotlin, Javascript, Typescript, Wasm, React Native, Go, C#, Rust, Flutter/Dart, Python)

For a more complete and detailed list of LWK features see [here](features.md).

## Get started

[Install LWK](install.md) and go through our [tutorial](basics.md).
