## LWK CLI

All LWK functions are exposed via a local JSON-RPC server that communicates with a CLI tool so you can see LWK in action.

This JSON-RPC Server also makes it easier to build your own frontend, GUI, or integration.

If you want to see an overview of LWK and a demo with the CLI tool check out this [video](https://community.liquid.net/c/videos/demo-liquid-wallet-kit-lwk)

### Installing LWK_CLI from crates.io

```shell
cargo install lwk_cli
```
or if you want to connect Jade over serial:

```shell
cargo install lwk_cli --features serial
```

### Building LWK_CLI from source

First you need [rust](https://www.rust-lang.org/tools/install), our MSRV is 1.85.0
then you can build from source:

```shell
git clone git@github.com:Blockstream/lwk.git
cd lwk
cargo install --path ./lwk_cli/
```

Or
```
cargo install --path ./lwk_cli/ --features serial
```
To enable connection with Jade over serial.

## Using LWK_CLI

Help will show available commands:

```shell
lwk_cli --help
```

Start the rpc server (default in Liquid Testnet)
and put it in background
```shell
lwk_cli server start
```
Every command requires the server running so open a new shell to run the client.

Create new BIP39 mnemonic for a software signer
```shell
lwk_cli signer generate
```
Load a software *signer* named `sw` from a given BIP39 mnemonic
```shell
lwk_cli signer load-software --signer sw --persist false --mnemonic "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about"
```

Create a p2wpkh *wallet* named `ss` (install [`jq`](https://github.com/jqlang/jq) or extract the descriptor manually)
```shell
DESC=$(lwk_cli signer singlesig-desc -signer sw --descriptor-blinding-key slip77 --kind wpkh | jq -r .descriptor)
lwk_cli wallet load --wallet ss -d $DESC
```

Get the wallet balance
```shell
lwk_cli wallet balance -w ss
```
If you have a Jade, you can plug it in and use it to create a
wallet and sign its transactions.

Probe connected Jades and prompt user to unlock it to get identifiers needed to load Jade on LWK

```shell
lwk_cli signer jade-id
```
Load Jade using returned ID

```shell
lwk_cli signer load-jade --signer <SET_A_NAME_FOR_THIS_JADE> --id <ID>
```
Get xpub from loaded Jade

```shell
lwk_cli signer xpub --signer <NAME_OF_THIS_JADE> --kind <bip84, bip49 or bip87>
```

When you're done, stop the rpc server.
```shell
lwk_cli server stop
```
