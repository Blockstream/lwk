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

Do a request
```
curl --header "Content-Type: application/json" --request POST --data '{"method":"generate_signer","params":[],"id":1,"jsonrpc":"2.0"}' http://localhost:32111 -s
```
