
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
mnemonic="abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about"
ctDesc=ks.SingleSigCtDesc(mnemonic)
print(ctDesc)
w=ks.Wollet(ks.ElementsNetwork.LIQUID_TESTNET(), ctDesc, "/tmp/ks")
w.sync()
w.balance()
# {'144c654344aa716d6f3abcc1ca90e5641e4e2a7f633bc09fe3baf64585819a49': 100000}
```
