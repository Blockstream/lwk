# CLI

Building the needed executable requires [rust](https://www.rust-lang.org/tools/install):

```sh
$ git clone git@github.com:Blockstream/lwk.git
$ cd lwk
$ cargo build --release
$ alias cli="$(pwd)/target/release/lwk_cli"
```

Help shows available commands:

```sh
$ cli --help
```

Install bash completion with:

```sh
$ cli generate-completion bash | jq -r . | sudo tee /usr/share/bash-completion/completions/cli
```

Other shell are available: bash, elvish, fish, powershell, zsh.
The destination file path `/usr/share/bash-completion/completions/cli` may change according to your distro.

## Server

### Start

```sh
$ cli server start
```

Or with more logs:


```sh
$ RUST_LOG=debug cli server start
```

Start the server in background and have logs on file

```sh
$ cli server start 2>debug.log &
```

### Stop

If not in background hit ctrl-c in the terminal where it started or in another shell:

```sh
$ cli server stop
```

Another way to terminate a server started in background is to type `fg` to bring the background
process in the forground and then hit `ctrl-c`

## Client

Every command requires the server running.

Generate a software signer ("stateless" request)

```sh
$ cli signer generate
```

is equivalent to:

```sh
$ curl --header "Content-Type: application/json" --request POST --data '{"method":"generate_signer","params":[],"id":1,"jsonrpc":"2.0"}' http://localhost:32111 -s
```

To see RPC data exchanged via the cli commands enable app log tracing eg:

```sh
$ RUST_LOG=app=trace cargo run -- wallet balance --wallet ciao
...
2023-11-28T09:36:18.696846Z TRACE app::client: ---> {"method":"balance","params":{"name":"ciao"},"id":2,"jsonrpc":"2.0"}
2023-11-28T09:36:18.697675Z TRACE app::client: <--- {"result":null,"error":{"code":-32008,"message":"Wallet 'ciao' does not exist","data":{"name":"ciao"}},"id":2,"jsonrpc":"2.0"}
{
  "code": -32008,
  "data": {
    "name": "ciao"
  },
  "message": "Wallet 'ciao' does not exist"
}
```


Load a wallet and request a balance ("stateful" request)


```sh
$ cli wallet load --wallet custody -d "ct(L3jXxwef3fpB7hcrFozcWgHeJCPSAFiZ1Ji2YJMPxceaGvy3PC1q,elwpkh(tpubD6NzVbkrYhZ4Was8nwnZi7eiWUNJq2LFpPSCMQLioUfUtT1e72GkRbmVeRAZc26j5MRUz2hRLsaVHJfs6L7ppNfLUrm9btQTuaEsLrT7D87/*))#lrwadl63"
$ cli wallet balance --wallet custody
```

is equivalent to:

```sh
$ curl --header "Content-Type: application/json" --request POST --data '{"method":"load_wallet","params":{"descriptor":"ct(L3jXxwef3fpB7hcrFozcWgHeJCPSAFiZ1Ji2YJMPxceaGvy3PC1q,elwpkh(tpubD6NzVbkrYhZ4Was8nwnZi7eiWUNJq2LFpPSCMQLioUfUtT1e72GkRbmVeRAZc26j5MRUz2hRLsaVHJfs6L7ppNfLUrm9btQTuaEsLrT7D87/*))#lrwadl63", "name": "custody"},"id":1,"jsonrpc":"2.0"}' http://localhost:32111 -s

$ curl --header "Content-Type: application/json" --request POST --data '{"method":"balance","params":{"name":"custody"},"id":1,"jsonrpc":"2.0"}' http://localhost:32111 -s | jq .result
```

Request an address:

```sh
$ cli wallet address --wallet custody
$ cli wallet address --wallet custody --index 4
```

An error test case:

```sh
$ curl --header "Content-Type: application/json" --request POST --data '{"method":"load_wallet","params":{"desc":"fake"},"id":1,"jsonrpc":"2.0"}' http://localhost:32111 -s | jq
{
  "jsonrpc": "2.0",
  "id": 1,
  "error": {
    "code": -32603,
    "message": "Serde JSON Error: missing field `descriptor`",
    "data": null
  }
}
```

### Create a singlesig wallet

First start the server
```sh
cli --network testnet server start
```

```sh
$ MNEMONIC=$(cli signer generate | jq -r .mnemonic)
$ cli signer load-software --mnemonic "$MNEMONIC" --signer s1
$ DESCRIPTOR=$(cli signer singlesig-desc --signer s1 --descriptor-blinding-key slip77 --kind wpkh | jq -r .descriptor)
$ cli wallet load --wallet w1 -d "$DESCRIPTOR"
$ cli wallet address --wallet w1
```

Send some lbtc to the address

```sh
$ cli wallet balance --wallet w1
```

Should show a balance

### Creating a transaction, signing and broadcasting

You must have a loaded singlesig wallet `w1`, with the corresponding signer `w1` as created in the previous step.
Must also have funds in the wallet.

```sh
$ UNSIGNED_PSET=$(cli wallet send --wallet w1 --recipient tlq1qqwe0a3dp3hce866snkll5vq244n47ph5zy2xr330uc8wkrvc0whwsvw4w67xksmfyxwqdyrykp0tsxzsm24mqm994pfy4f6lg:1000:144c654344aa716d6f3abcc1ca90e5641e4e2a7f633bc09fe3baf64585819a49 | jq -r .pset)
```

Creates an unsigned PSET sending 1000 satoshi of liquid btc (144c654344aa716d6f3abcc1ca90e5641e4e2a7f633bc09fe3baf64585819a49 is the policy asset in testnet) to the address tlq1qqwe0a3dp3hce866snkll5vq244n47ph5zy2xr330uc8wkrvc0whwsvw4w67xksmfyxwqdyrykp0tsxzsm24mqm994pfy4f6lg


Sign the pset

```sh
$ SIGNED_PSET=$(cli signer sign --signer s1 --pset $UNSIGNED_PSET | jq -r .pset)
```

Broadcast it. Remove `--dry-run` to effectively broadcast live, otherwise only partial checks on the transactions finalization are made (for example it's not checked inputs are unspent)

```sh
$ cli wallet broadcast --dry-run --wallet w1 --pset $SIGNED_PSET)

```
