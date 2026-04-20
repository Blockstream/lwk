import type { HarnessRawEnvelope } from "../../src/url.js";

// Source: green_android/data/.../WalletAbiDemoRequestSource.kt
// This is the current static split-style demo envelope used by Green.
export const GREEN_ANDROID_DEMO_SPLIT_ENVELOPE: HarnessRawEnvelope = {
  jsonrpc: "2.0",
  id: "wallet-abi-demo-envelope",
  method: "wallet_abi_process_request",
  params: {
    abi_version: "wallet-abi-0.1",
    request_id: "wallet-abi-demo-request",
    network: "testnet-liquid",
    params: {
      inputs: [],
      outputs: [
        {
          id: "output-1",
          amount_sat: 1000,
          lock: {
            type: "script",
            script: "00140000000000000000000000000000000000000000",
          },
          asset: {
            asset_id:
              "144c654344aa716d6f3abcc1ca90e5641e4e2a7f633bc09fe3baf64585819a49",
          },
          blinder: {
            type: "rand",
          },
        },
        {
          id: "output-2",
          amount_sat: 2000,
          lock: {
            type: "script",
            script: "00141111111111111111111111111111111111111111",
          },
          asset: {
            asset_id:
              "144c654344aa716d6f3abcc1ca90e5641e4e2a7f633bc09fe3baf64585819a49",
          },
          blinder: {
            type: "rand",
          },
        },
      ],
      fee_rate_sat_kvb: 12000,
    },
    broadcast: true,
  },
};

// Source: green_android/androidApp/.../WalletAbiHappyPathTest.kt issuanceEnvelope(...)
export const GREEN_ANDROID_ISSUANCE_ENVELOPE: HarnessRawEnvelope = {
  jsonrpc: "2.0",
  id: "wallet-abi-issuance-envelope",
  method: "wallet_abi_process_request",
  params: {
    abi_version: "wallet-abi-0.1",
    request_id: "wallet-abi-demo-request",
    network: "testnet-liquid",
    broadcast: true,
    params: {
      inputs: [
        {
          id: "issuance",
          utxo_source: {
            wallet: {
              filter: {
                asset: "none",
                amount: "none",
                lock: "none",
              },
            },
          },
          unblinding: "wallet",
          sequence: 4294967295,
          issuance: {
            kind: "new",
            asset_amount_sat: 5,
            token_amount_sat: 1,
            entropy: new Array(32).fill(7),
          },
          finalizer: {
            type: "wallet",
          },
        },
      ],
      outputs: [
        {
          id: "issued_asset",
          amount_sat: 5,
          lock: {
            type: "wallet",
          },
          asset: {
            type: "new_issuance_asset",
            input_index: 0,
          },
          blinder: "wallet",
        },
        {
          id: "reissuance_token",
          amount_sat: 1,
          lock: {
            type: "wallet",
          },
          asset: {
            type: "new_issuance_token",
            input_index: 0,
          },
          blinder: "wallet",
        },
      ],
    },
  },
};

// Source: green_android/androidApp/.../WalletAbiHappyPathTest.kt reissuanceEnvelope(...)
// The token asset id here is intentionally copied as-is from Green's Android test vector.
export const GREEN_ANDROID_REISSUANCE_ENVELOPE: HarnessRawEnvelope = {
  jsonrpc: "2.0",
  id: "wallet-abi-reissuance-envelope",
  method: "wallet_abi_process_request",
  params: {
    abi_version: "wallet-abi-0.1",
    request_id: "wallet-abi-demo-request",
    network: "testnet-liquid",
    broadcast: true,
    params: {
      inputs: [
        {
          id: "reissuance",
          utxo_source: {
            wallet: {
              filter: {
                asset: {
                  exact: {
                    asset_id: "reissuance_token_asset",
                  },
                },
                amount: "none",
                lock: "none",
              },
            },
          },
          unblinding: "wallet",
          sequence: 4294967295,
          issuance: {
            kind: "reissue",
            asset_amount_sat: 7,
            token_amount_sat: 0,
            entropy: new Array(32).fill(8),
          },
          finalizer: {
            type: "wallet",
          },
        },
      ],
      outputs: [
        {
          id: "reissued_asset",
          amount_sat: 7,
          lock: {
            type: "wallet",
          },
          asset: {
            type: "re_issuance_asset",
            input_index: 0,
          },
          blinder: "wallet",
        },
      ],
    },
  },
};
