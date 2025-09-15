# Features

* **Watch-Only** wallet support: using Liquid descriptors, better known as
  [CT descriptors](https://github.com/ElementsProject/ELIPs/blob/main/elip-0150.mediawiki).
* **PSET** based: transactions are shared and processed using the
  [Partially Signed Elements Transaction](https://github.com/ElementsProject/elements/blob/1fcf0cf2323b7feaff5d1fc4c506fff5ec09132e/doc/pset.mediawiki) format.
* **Electrum** and **Esplora** [backends](https://github.com/Blockstream/electrs):
  no need to run and sync a full Liquid node or rely on closed source servers.
* **Asset issuance**, **reissuance** and **burn** support: manage the lifecycle
  of your Issued Assets with a lightweight client.
* **Generic multisig** wallets: create a wallet controlled by
  any combination of hardware or software signers, with a user
  specified quorum.
* **Hardware signer** support: receive, issue, reissue and burn L-BTC and
  Issued Assets with your hardware signer, using singlesig or multisig
  wallets (currently [**Jade**](https://blockstream.com/jade/) only, with more coming soon).
* **Native bindings** [PoC support](./lwk_bindings#readme) for Python, Kotlin and Swift, with many other language available soon using [uniffi](https://mozilla.github.io/uniffi-rs/)
* **WASM** [`lwk_wasm`](./lwk_wasm) crate, see it in action in the [Liquid Web Wallet](https://liquidwebwallet.org/).
* **JSON-RPC Server** support: all functions are exposed via JSON-RPC Server, making it easier to build your own frontend, GUI, or integration.
