# LWK npm workspace

This directory is the private npm workspace used to publish:

All packages in this workspace are dev builds for PoC testing only. Do not treat them as production-ready releases.

- `lwk_node`
- `wallet_abi_sdk_core_web`
- `helpers_wallet_abi_web`
- `lwk_wallet_abi_sdk`

Package roles:

- `wallet_abi_sdk_core_web`: raw browser-target `wasm-pack` projection of the `lwk_wasm` crate
- `lwk_node`: raw Node.js-target `wasm-pack` projection of the same crate
- `helpers_wallet_abi_web`: thin web-only Wallet ABI wrapper over `wallet_abi_sdk_core_web`
- `lwk_wallet_abi_sdk`: headless browser Wallet ABI SDK over `helpers_wallet_abi_web`

The wrapper layers stay split on purpose:

- `helpers_wallet_abi_web` adds typed Wallet ABI imports and a few schema helpers only
- `lwk_wallet_abi_sdk` adds protocol helpers, request builders, a typed client, and WalletConnect session transport

Still out of scope for this workspace:

- React providers and hooks
- page-level application state
- wallet-side native provider bridges

Workspace development:

```sh
npm ci
npm run build
npm run test
```

These commands validate all published workspace packages.

Package tarball checks:

```sh
npm run pack:check
```
