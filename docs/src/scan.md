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
