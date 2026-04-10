# `helpers_wallet_abi_web`

This package is a dev build for PoC testing only.

Thin web-only Wallet ABI helpers for `wallet_abi_sdk_core_web`.

```sh
npm install helpers_wallet_abi_web
```

```ts
import {
  Network,
  WalletAbiCapabilities,
  loadLwkWalletAbiWeb,
} from "helpers_wallet_abi_web";
import { networkFromString } from "helpers_wallet_abi_web/helpers";
import { WalletAbiTxCreateRequest } from "helpers_wallet_abi_web/schema";

await loadLwkWalletAbiWeb();

const network: Network = networkFromString("liquid-testnet");
const capabilities = WalletAbiCapabilities.new(network, [
  "wallet_abi_process_request",
]);
const requestType = WalletAbiTxCreateRequest;

void capabilities;
void requestType;
```

This package keeps `wallet_abi_sdk_core_web` as the raw wasm projection and adds only:

- typed Wallet ABI schema re-exports
- a memoized wasm loader
- small string and network conversion helpers

Exports:

- `helpers_wallet_abi_web`
- `helpers_wallet_abi_web/schema`
- `helpers_wallet_abi_web/helpers`

Out of scope:

- provider bridges
- JSON-RPC requester/client logic
- WalletConnect session handling
