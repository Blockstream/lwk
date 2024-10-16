
## Bindings

To generate bindings the projects use [Mozilla uniffi](https://mozilla.github.io/uniffi-rs/) giving support for: Kotlin, Swift, Python, Ruby and also third party support for Kotlin multiplatform, Go, C++, C# and Dart.

There is an architectural refactor already planned for the crates the bindings are created on, this initial version is for experimentation only, 
expect **breaking changes** in the API

Most of the rust types in this crate are wrappers on types in [`lwk_wollet`] and [`lwk_signer`] which satisfy uniffi requirements such as:
* Methods on types support only `&self`, thus if the inner type needs mutability, it is usually enclosed in a [`std::sync::Mutex`].
* Returned values must be wrapped in [`std::sync::Arc`] so that there aren't issue in memory management.

## Host & Requirements

Build supported on Mac and Linux.

Note the following commands requires some env var defined in `../context/env.sh`. If you use `direnv` and allowed the `.envrc` file they are automatically evaluated when entering the dir, otherwise launch manually via `. ./context/env.sh`

For android build you need the NDK greater than r23 in `${PROJECT_DIR}/bin/android-ndk`, if you already have it elsewhere just symlink your path.

Building bindings requires launching commands with many arguments, [just](https://github.com/casey/just) tool is used for that.
It's a simple make-like tool, you can either install the tool or copy-paste the shell commands inside it.

## Python

### Examples

* [List transactions](./tests/bindings/list_transactions.py) of a wpkh/slip77 wallet
* [Send transaction](./tests/bindings/send_transaction.py) of a wpkh/slip77 wallet in a regtest environment
* [Send asset](./tests/bindings/send_asset.py) of a wpkh/slip77 wallet in a regtest environment
* [Custom persister](./tests/bindings/custom_persister.py), the caller code provide how the wallet updates are persisted

### Build Python wheel

First, create a virtual env, skip the step if you already created it.

```shell
cd lwk/lwk_bindings
virtualenv venv
source venv/bin/activate
pip install maturin maturin[patchelf] uniffi-bindgen==0.28.0
```

Then build the wheel

```shell
cd lwk/lwk_bindings
maturin develop
```

Try it (note there is still an issue in how we import the package when using the wheel):

```python
import lwk
str(lwk.Network.mainnet())
```

### Publish Python wheel

Download artifacts from CI:

 - github ci for windows and mac
 - gitlab ci for linux

```sh
$ twine upload *.whl
```

### Test

```shell
cargo test -p lwk_bindings --features foreign_bindings --test bindings -- py
```

Live environment

```shell
just python-env-bindings
```

## Kotlin


### Example

* [List transactions](./tests/bindings/list_transactions.kts) of a wpkh/slip77 wallet


### Build

This will build the bindings library in debug mode and generate the kotlin file

```shell
just kotlin
```

Create android bindings library libs, 4 architectures in release mode

```shell
just android
```

## Swift

### Example

* [List transactions](./tests/bindings/list_transactions.swift) of a wpkh/slip77 wallet
