# `@blockstream/lwk-web`

Liquid Wallet Kit for browser and bundler environments via WebAssembly.

```sh
npm install @blockstream/lwk-web
```

```ts
import * as lwk from "@blockstream/lwk-web";

const network = lwk.Network.testnet();
const signer = new lwk.Signer(lwk.Mnemonic.fromRandom(12), network);
```

For Vite projects, add
[`vite-plugin-wasm`](https://www.npmjs.com/package/vite-plugin-wasm) and
[`vite-plugin-top-level-await`](https://www.npmjs.com/package/vite-plugin-top-level-await).
