# LWK npm workspace

This directory is the private npm workspace used to publish:

- `lwk_node`
- `lwk_web`
- `lwk_wallet_abi_web`

Package roles:

- `lwk_web`: raw browser-target `wasm-pack` projection of the `lwk_wasm` crate
- `lwk_node`: raw Node.js-target `wasm-pack` projection of the same crate
- `lwk_wallet_abi_web`: thin web-only Wallet ABI wrapper over `lwk_web`

The wrapper package adds typed Wallet ABI imports and a few schema helpers only.
Provider transport, requester logic, and WalletConnect session handling remain
outside this workspace package.

Workspace development:

```sh
npm ci
npm run build
npm run test
```

These commands validate both published workspace packages.

Package tarball checks:

```sh
npm run pack:check
```
