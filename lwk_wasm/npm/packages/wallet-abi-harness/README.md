# `wallet_abi_harness`

Private local-only Wallet ABI playground for:

- typed scenario building
- JSON preview and shareable URLs
- WalletConnect pairing and request transport
- deterministic fixture-backed Vitest coverage
- manual Green live smoke for transfer and split flows

Run it from the npm workspace root:

```sh
npm run dev -w wallet_abi_harness
```

Run the harness tests:

```sh
npm run test -w wallet_abi_harness
```

Manual live smoke against Green:

```sh
cd lwk_wasm/npm
VITE_WALLETCONNECT_PROJECT_ID=your_project_id npm run dev -w wallet_abi_harness
```

Then:

1. Open the harness in a browser and leave the scenario mode in `walletconnect`.
2. Pair a Green development build from the Transact tab or an external `wc:` deep link.
3. Run `get_signer_receive_address`, `get_raw_signing_x_only_pubkey`, one `transfer`, and one `split`.
4. Use the transcript panel as the source of truth for the outbound request, inbound response, and active session topic.

`VITE_WALLETCONNECT_PROJECT_ID` pre-fills the WalletConnect project id field, but you can still override it in the UI. Shareable `#scenario=` URLs remain versioned, and the raw envelope editor stays available as an expert override path.
