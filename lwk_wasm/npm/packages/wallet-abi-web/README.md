# `lwk_wallet_abi_web`

Thin web-only Wallet ABI helpers for `lwk_web`.

```sh
npm install lwk_wallet_abi_web
```

```ts
import {
  Network,
  WalletAbiCapabilities,
  loadLwkWalletAbiWeb,
} from "lwk_wallet_abi_web";
import { networkFromString } from "lwk_wallet_abi_web/helpers";
import { WalletAbiTxCreateRequest } from "lwk_wallet_abi_web/schema";

await loadLwkWalletAbiWeb();

const network: Network = networkFromString("liquid-testnet");
const capabilities = WalletAbiCapabilities.new(network, [
  "wallet_abi_process_request",
]);
const requestType = WalletAbiTxCreateRequest;

void capabilities;
void requestType;
```

This package keeps `lwk_web` as the raw wasm projection and adds only:

- typed Wallet ABI schema re-exports
- a memoized wasm loader
- small string and network conversion helpers

Exports:

- `lwk_wallet_abi_web`
- `lwk_wallet_abi_web/schema`
- `lwk_wallet_abi_web/helpers`

Out of scope:

- provider bridges
- JSON-RPC requester/client logic
- WalletConnect session handling
