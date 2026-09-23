# C#

## Examples

C# bindings use .NET 8 and C# 12; they are very immature at the moment:

- They use a [third-party UniFFI bindings generator](https://github.com/NordSecurity/uniffi-bindgen-cs) compatible with UniFFI 0.29.x
- It's currently tested only in linux
- The dynamic library is referenced in a non-standard way

* [List transactions](../../lwk_bindings/tests/bindings/list_transactions.cs) of a wpkh/slip77 wallet
