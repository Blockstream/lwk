# CLI

TODO

Start the server
```
cli server
```

Read the logs
```
tail -f debug.log
```

Do a "stateless" request
```
curl --header "Content-Type: application/json" --request POST --data '{"method":"generate_signer","params":[],"id":1,"jsonrpc":"2.0"}' http://localhost:32111 -s
```

Do a "stateful" request
```
$ curl --header "Content-Type: application/json" --request POST --data '{"method":"load_wallet","params":{"descriptor":"ct(L3jXxwef3fpB7hcrFozcWgHeJCPSAFiZ1Ji2YJMPxceaGvy3PC1q,elwpkh(tpubD6NzVbkrYhZ4Was8nwnZi7eiWUNJq2LFpPSCMQLioUfUtT1e72GkRbmVeRAZc26j5MRUz2hRLsaVHJfs6L7ppNfLUrm9btQTuaEsLrT7D87/*))#lrwadl63"},"id":1,"jsonrpc":"2.0"}' http://localhost:32111 -s | jq .result.new
true
$ curl --header "Content-Type: application/json" --request POST --data '{"method":"load_wallet","params":{"descriptor":"ct(L3jXxwef3fpB7hcrFozcWgHeJCPSAFiZ1Ji2YJMPxceaGvy3PC1q,elwpkh(tpubD6NzVbkrYhZ4Was8nwnZi7eiWUNJq2LFpPSCMQLioUfUtT1e72GkRbmVeRAZc26j5MRUz2hRLsaVHJfs6L7ppNfLUrm9btQTuaEsLrT7D87/*))#lrwadl63"},"id":1,"jsonrpc":"2.0"}' http://localhost:32111 -s | jq .result.new
false
```
