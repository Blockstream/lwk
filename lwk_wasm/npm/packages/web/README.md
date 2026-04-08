# `lwk_web`

Liquid Wallet Kit for browsers via WebAssembly.

```sh
npm install lwk_web
```

```ts
import * as lwk from "lwk_web";

const network = lwk.Network.testnet();
```

Use this package with a bundler that can load WebAssembly modules from ESM
imports.
