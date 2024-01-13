
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

```python
import ks_bindings as ks

mnemonic = ks.Mnemonic("abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about")
network = ks.NetworkBuilder().testnet()

signer = ks.Signer(mnemonic, network)
desc = signer.wpkh_slip77_descriptor()

print(desc) 
# ct(slip77(9c8e4f05c7711a98c838be228bcb84924d4570ca53f35fa1c793e58841d47023),elwpkh([73c5da0a/84'/1'/0']tpubDC8msFGeGuwnKG9Upg7DM2b4DaRqg3CUZa5g8v2SRQ6K4NSkxUgd7HsL2XVWbVm39yBA4LAxysQAm397zwQSQoQgewGiYZqrA9DsP4zbQ1M/<0;1>/*))#2e4n992d

w = ks.Wollet(network, desc, "/tmp/ks", network.default_electrum_url())
w.sync()

w.balance()
# {'144c654344aa716d6f3abcc1ca90e5641e4e2a7f633bc09fe3baf64585819a49': 100000}

[str(tx.txid()) for tx in w.transactions()]

```
