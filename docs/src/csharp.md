# C#

## Examples

C# bindings use .NET 8 and C# 12; they are very immature at the moment:

- They use a uniffi bindings generator from a [third party](https://github.com/NordSecurity/uniffi-bindgen-cs) which didn't yet ship for uniffi 0.28 
- It's currently tested only in linux
- The dynamic library is referenced in a non-standard way

* [List transactions](../../lwk_bindings/tests/bindings/list_transactions.cs) of a wpkh/slip77 wallet
