# `lwk_node`

Liquid Wallet Kit for Node.js via WebAssembly.

```sh
npm install lwk_node
```

```ts
import * as lwk from "lwk_node";

const network = lwk.Network.testnet();
```

For async setup APIs, use the static async factories:

```ts
const jade = await lwk.Jade.fromSerial(network, true);
```

## Running tests locally

Tun run tests locally use the following command:
```bash
npm run test
```

Note: when creating a new test, make sure to make the desired function with `export default` the the script file can find it, use other tests as an example.
If no default export is found, it falls back to executing the file itself.