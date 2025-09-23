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
