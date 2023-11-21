# CLI

Building the needed executable requires [rust](https://www.rust-lang.org/tools/install):

```sh
$ git clone git@gl.blockstream.io:leocomandini/bewallet.git keystone
$ cd keystone
$ cargo build --release
$ alias cli="$(pwd)/target/release/cli"
```

Help shows available commands:

```sh
$ cli --help
```

## Server

Start the server

```sh
$ cli server start
```

Stop the server

Hit ctrl-c on the terminal window where it started or

```sh
$ cli server stop
```

Read the logs

```sh
$ tail -f debug.log
```

Or launch printing on stderr with:

```sh
$ cli --stderr server start
```

With more logs

```sh
$ RUST_LOG=debug cli --stderr server start
```

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

Load a wallet and request a balance ("stateful" request)


```sh
$ cli wallet load --name custody "ct(L3jXxwef3fpB7hcrFozcWgHeJCPSAFiZ1Ji2YJMPxceaGvy3PC1q,elwpkh(tpubD6NzVbkrYhZ4Was8nwnZi7eiWUNJq2LFpPSCMQLioUfUtT1e72GkRbmVeRAZc26j5MRUz2hRLsaVHJfs6L7ppNfLUrm9btQTuaEsLrT7D87/*))#lrwadl63"
$ cli wallet balance --name custody
```

is equivalent to:

```sh
$ curl --header "Content-Type: application/json" --request POST --data '{"method":"load_wallet","params":{"descriptor":"ct(L3jXxwef3fpB7hcrFozcWgHeJCPSAFiZ1Ji2YJMPxceaGvy3PC1q,elwpkh(tpubD6NzVbkrYhZ4Was8nwnZi7eiWUNJq2LFpPSCMQLioUfUtT1e72GkRbmVeRAZc26j5MRUz2hRLsaVHJfs6L7ppNfLUrm9btQTuaEsLrT7D87/*))#lrwadl63", "name": "custody"},"id":1,"jsonrpc":"2.0"}' http://localhost:32111 -s

$ curl --header "Content-Type: application/json" --request POST --data '{"method":"balance","params":{"name":"custody"},"id":1,"jsonrpc":"2.0"}' http://localhost:32111 -s | jq .result
```

Request an address:

```sh
$ cli wallet address --name custody
$ cli wallet address --name custody --index 4
```

An error test case:

```sh
$ curl --header "Content-Type: application/json" --request POST --data '{"method":"load_wallet","params":{"desc":"fake"},"id":1,"jsonrpc":"2.0"}' http://localhost:32111 -s | jq
{
  "jsonrpc": "2.0",
  "id": 1,
  "error": {
    "code": -32603,
    "message": "Internal error",
    "data": "Serde JSON Error: missing field `descriptor`"
  }
}
```

### Create a singlesig wallet

By default will create a liquid testnet wallet

```sh
$ MNEMONIC=$(cli signer generate | jq -r .mnemonic)
$ cli signer load --kind software --mnemonic "$MNEMONIC" --name s1
$ DESCRIPTOR=$(cli signer singlesig-descriptor --name s1 --descriptor-blinding-key slip77 --kind wpkh | jq -r .descriptor)
$ cli wallet load --name w1 "$DESCRIPTOR"
$ cli wallet address --name w1
```

Send some lbtc to the address

```sh
$ cli wallet balance --name w1
```

Should show a balance

### Creating a transaction, signing and broadcasting

You must have a loaded singlesig wallet `w1`, with the corresponding signer `w1` as created in the previous step.
Must also have funds in the wallet.

```sh
$ UNSIGNED_PSET=$(cli wallet send --name w1 --recipient tlq1qqwe0a3dp3hce866snkll5vq244n47ph5zy2xr330uc8wkrvc0whwsvw4w67xksmfyxwqdyrykp0tsxzsm24mqm994pfy4f6lg:1000:144c654344aa716d6f3abcc1ca90e5641e4e2a7f633bc09fe3baf64585819a49 | jq -r .pset)
```

Creates an unsigned PSET sending 1000 satoshi of liquid btc (144c654344aa716d6f3abcc1ca90e5641e4e2a7f633bc09fe3baf64585819a49 is the policy asset in testnet) to the address tlq1qqwe0a3dp3hce866snkll5vq244n47ph5zy2xr330uc8wkrvc0whwsvw4w67xksmfyxwqdyrykp0tsxzsm24mqm994pfy4f6lg


Sign the pset

```sh
$ SIGNED_PSET=$(cli signer sign --name s1 $UNSIGNED_PSET)
```

Broadcast it. Remove `--dry-run` to effectively broadcast live, otherwise only partial checks on the transactions finalization are made (for example it's not checked inputs are unspent)

```sh
$ cli wallet broadcast --dry-run --name s1 $SIGNED_PSET)

```