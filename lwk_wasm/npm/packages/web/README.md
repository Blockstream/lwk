# `wallet_abi_sdk_core_web`

This package is a dev build for PoC testing only.

Liquid Wallet Kit for browsers via WebAssembly.

```sh
npm install wallet_abi_sdk_core_web
```

```ts
import * as lwk from "wallet_abi_sdk_core_web";

const network = lwk.Network.testnet();
```

Use this package with a bundler that can load WebAssembly modules from ESM
imports.
