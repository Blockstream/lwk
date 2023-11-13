# CLI

Building the needed executable requires [rust](https://www.rust-lang.org/tools/install):

```
$ git clone git@gl.blockstream.io:leocomandini/bewallet.git keystone
$ cd keystone
$ cargo build --release
$ alias cli="$(pwd)/target/release/cli"
```

Help shows available commands:

```
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
