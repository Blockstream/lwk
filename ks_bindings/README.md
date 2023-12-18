
## Bindings

To generate bindings the projects use [Mozilla uniffi](https://mozilla.github.io/uniffi-rs/) giving support for: Kotlin, Swift, Python, Ruby and also third party support for Kotlin, Go, C# and Dart.

There is an architectural refactor already planned for the crates the bindings are created on, this initial version is for experimentation only, 
expect **breaking changes** in the API

Building bindings requires launching commands with many arguments, [just](https://github.com/casey/just) tool is used for that.
It's a simple make-like tool, you can either install the tool or copy-paste the shell commands inside it.


### Build

```shell
just build-bindings
```

### Test python bindings

```sh
just env-bindings
```

...automatically starts a python shell...

```python
>>> import ks_bindings as ks
>>> w=ks.Wollet(ks.ElementsNetwork.LIQUID_TESTNET(), "ct(L3jXxwef3fpB7hcrFozcWgHeJCPSAFiZ1Ji2YJMPxceaGvy3PC1q,elwpkh(tpubD6NzVbkrYhZ4Was8nwnZi7eiWUNJq2LFpPSCMQLioUfUtT1e72GkRbmVeRAZc26j5MRUz2hR
LsaVHJfs6L7ppNfLUrm9btQTuaEsLrT7D87/*))#lrwadl63", "/tmp/")
>>> w.sync()
>>> w.balance()
{'144c654344aa716d6f3abcc1ca90e5641e4e2a7f633bc09fe3baf64585819a49': 100000}
```
