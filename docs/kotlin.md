# Kotlin

## Build

This will build the bindings library in debug mode and generate the kotlin file

```shell
just kotlin
```

Create android bindings library libs, 4 architectures in release mode

```shell
just android
```

## Examples

* [List transactions](./lwk_bindings/tests/bindings/list_transactions.kts) of a wpkh/slip77 wallet
