# `@blockstream/lwk-node`

Liquid Wallet Kit for Node.js via WebAssembly.

```sh
npm install @blockstream/lwk-node
```

```ts
import * as lwk from "@blockstream/lwk-node";

const network = lwk.Network.testnet();
```

For async setup APIs, use the static async factories:

```ts
const jade = await lwk.Jade.fromSerial(network, true);
```
