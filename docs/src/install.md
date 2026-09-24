# Installing LWK

LWK is available for several languages.

## Rust
You can use the crates released on [crates.io](https://crates.io)

```rust,ignore
[dependencies]
lwk_wollet = "0.19.0"
lwk_signer = "0.19.0"
lwk_common = "0.19.0"
```

## Python
You can use the official python package: [lwk](https://pypi.org/project/lwk/)

```shell,ignore
pip install lwk
```


## Javascript/Typescript (Wasm)

### Wasm module

Install LWK
```shell,ignore
npm install lwk_wasm
```

Import LWK
```typescript,ignore
const lwk = require('lwk_wasm');
```

### Node module

Install LWK
```shell,ignore
npm install lwk_node
```

Import LWK
```typescript,ignore
const lwk = require('lwk_node');
```

## iOS/Swift

Build the Swift bindings and XCFramework from source on macOS:

```shell,ignore
just swift
```

The generated framework is written to `target/lwkFFI.xcframework`.

## Android/Kotlin

Build the Android native libraries and generate Kotlin bindings from source:

```shell,ignore
just android
just kotlin
```

## React Native

LWK does not currently publish first-party React Native bindings.

## Go

Generate the Go bindings and native library from source:

```shell,ignore
just go-build-bindings
```

## C#

```shell,ignore
dotnet add package LiquidWalletKit --version 0.8.2
```

Please open an issue if you need a more recent version

## Flutter/Dart

LWK does not currently publish first-party Flutter or Dart bindings.
Community-maintained bindings are available from
[`lwk-dart`](https://github.com/SatoshiPortal/lwk-dart) and published as the
[`lwk` package](https://pub.dev/packages/lwk).
