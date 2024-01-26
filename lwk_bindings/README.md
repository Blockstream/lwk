
## Bindings

To generate bindings the projects use [Mozilla uniffi](https://mozilla.github.io/uniffi-rs/) giving support for: Kotlin, Swift, Python, Ruby and also third party support for Kotlin multiplatform, Go, C++, C# and Dart.

There is an architectural refactor already planned for the crates the bindings are created on, this initial version is for experimentation only, 
expect **breaking changes** in the API

Building bindings requires launching commands with many arguments, [just](https://github.com/casey/just) tool is used for that.
It's a simple make-like tool, you can either install the tool or copy-paste the shell commands inside it.

## Host

Build supported on Mac and Linux.

Note the following commands requires some env var defined in `../context/env.sh`. If you use `direnv` and allowed the `.envrc` file they are automatically evaluated when entering the dir, otherwise launch manually via `. ./context/env.sh`

For android build you need the NDK greater than r23 in `${PROJECT_DIR}/bin/android-ndk`, if you already have it elsewhere just symlink your path.

## Python bindings

### Example

* [List transactions](./tests/bindings/list_transactions.py) of a wpkh/slip77 wallet


### Test

```
cargo test -p lwk_bindings --features foreign_bindings --test bindings -- py
```

Live environment

```sh
just env-python-bindings
```

## Kotlin for Android

### Build

This will build the bindings library in debug mode and generate the kotlin file

```shell
just kotlin
```

Create android bindings library libs, 4 architectures in release mode

```shell
just android
```
