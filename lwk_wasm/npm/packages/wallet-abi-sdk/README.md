# `lwk_wallet_abi_sdk`

Browser-only headless Wallet ABI SDK built on top of `lwk_wallet_abi_web`.

```sh
npm install lwk_wallet_abi_sdk
```

This package keeps the lower layers split clean:

- `lwk_web`: raw generated wasm projection
- `lwk_wallet_abi_web`: typed Wallet ABI schema and small helpers
- `lwk_wallet_abi_sdk`: protocol helpers, typed client, request builders, and WalletConnect session transport

Current v1 scope:

- `get_signer_receive_address`
- `get_raw_signing_x_only_pubkey`
- `wallet_abi_process_request`

Exports:

- `lwk_wallet_abi_sdk`
- `lwk_wallet_abi_sdk/schema`
- `lwk_wallet_abi_sdk/helpers`
- `lwk_wallet_abi_sdk/protocol`
- `lwk_wallet_abi_sdk/client`
- `lwk_wallet_abi_sdk/builders`
- `lwk_wallet_abi_sdk/walletconnect`

## Network naming

The SDK owns the translation between the three network name layers used in the browser stack:

- `lwk_wallet_abi_web` / `Network.toString()`: `liquid`, `liquid-testnet`, `liquid-regtest`
- Wallet ABI transport names: `liquid`, `testnet-liquid`, `localtest-liquid`
- WalletConnect chains: `walabi:liquid`, `walabi:testnet-liquid`, `walabi:localtest-liquid`

Use the protocol and WalletConnect helpers instead of hard-coding those translations in app code.

## Example

```ts
import {
  WalletAbiClient,
  createTxCreateRequest,
  createWalletAbiSessionController,
  createWalletConnectRequester,
  loadLwkWalletAbiWeb,
  networkFromString,
} from "lwk_wallet_abi_sdk";

await loadLwkWalletAbiWeb();

const controller = await createWalletAbiSessionController({
  projectId: "<walletconnect-project-id>",
  network: "testnet-liquid",
  appUrl: "https://example.com",
  metadata: {
    name: "Example App",
  },
});

const requester = createWalletConnectRequester({
  chainId: controller.chainId,
  getTopic: () => controller.session()?.topic,
  client: {
    connect: () => controller.connect(),
    disconnect: () => controller.disconnect(),
    request: ({ request }) => controller.request(request),
  },
});

const client = new WalletAbiClient({ requester });
const address = await client.getSignerReceiveAddress();
const xonly = await client.getRawSigningXOnlyPubkey();

const network = networkFromString("liquid-testnet");
const request = createTxCreateRequest({
  network,
  params: /* WalletAbiRuntimeParams */,
});
const response = await client.processRequest(request);

void address;
void xonly;
void response;
```

## Builder surface

The SDK builders stay intentionally narrow and always return typed Wallet ABI classes:

- `generateRequestId()`
- `createWalletInput(...)`
- `createProvidedInput(...)`
- `createTxCreateRequest(...)`

They are request-assembly helpers only. They do not add app-specific DSLs or duplicate the schema classes from `lwk_wallet_abi_web`.

## WalletConnect notes

The SDK includes headless WalletConnect helpers for:

- CAIP chain mapping
- required namespace construction
- metadata creation
- JSON-RPC requester adaptation
- session restore and stale-session cleanup
- approval fallback and lifecycle subscriptions

`createWalletAbiSessionController(...)` accepts:

- `projectId`
- `network`
- `appUrl`
- optional `metadata`
- optional `storagePrefix`

Branding, secure-origin requirements, and runtime config loading remain caller concerns.

## Out of scope

- React hooks, contexts, and components
- wallet-side native provider bridges
- app-local state management
- `wallet_abi_get_capabilities`
- `wallet_abi_evaluate_request`

