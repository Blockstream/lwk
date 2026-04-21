import { beforeAll, describe, expect, test } from "vitest";

import { loadLwkWalletAbiWeb, networkFromString } from "lwk_wallet_abi_sdk";

import {
  DEFAULT_SPLIT_ADDRESS,
  DEFAULT_TRANSFER_ADDRESS,
  FIXTURE_REISSUANCE_TOKEN_ASSET,
  TESTNET_POLICY_ASSET,
  createDefaultScenario,
} from "../src/scenario.js";
import { scenarioToProcessRequestEnvelope } from "../src/request.js";

describe("request compilation", () => {
  beforeAll(async () => {
    await loadLwkWalletAbiWeb();
  });

  test("builds an actual transfer request", async () => {
    const envelope = await scenarioToProcessRequestEnvelope(
      createDefaultScenario("transfer"),
      11,
    );

    expect(envelope).toMatchObject({
      id: 11,
      jsonrpc: "2.0",
      method: "wallet_abi_process_request",
      params: {
        network: "testnet-liquid",
        broadcast: true,
        params: {
          inputs: [],
          fee_rate_sat_kvb: 1000,
          outputs: [
            {
              id: "output-1",
              amount_sat: 700,
              lock: {
                type: "script",
                script: "0014d0c4a3ef09e997b6e99e397e518fe3e41a118ca1",
              },
              asset: {
                asset_id: TESTNET_POLICY_ASSET,
              },
              blinder: "explicit",
            },
          ],
        },
      },
    });
  });

  test("builds an actual split request", async () => {
    const envelope = await scenarioToProcessRequestEnvelope(
      createDefaultScenario("split"),
      12,
    );
    const lwk = await loadLwkWalletAbiWeb();
    const network = networkFromString("liquid-testnet");
    const firstScript = lwk.Address.parse(DEFAULT_TRANSFER_ADDRESS, network)
      .scriptPubkey()
      .toString();
    const secondScript = lwk.Address.parse(DEFAULT_SPLIT_ADDRESS, network)
      .scriptPubkey()
      .toString();

    expect(envelope.params).toMatchObject({
      network: "testnet-liquid",
      params: {
        outputs: [
          {
            id: "output-1",
            amount_sat: 700,
            lock: {
              type: "script",
              script: firstScript,
            },
          },
          {
            id: "output-2",
            amount_sat: 900,
            lock: {
              type: "script",
              script: secondScript,
            },
          },
        ],
      },
    });
  });

  test("builds an actual issuance request", async () => {
    const envelope = await scenarioToProcessRequestEnvelope(
      createDefaultScenario("issuance"),
      13,
    );

    expect(envelope.params).toMatchObject({
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
            asset: {
              type: "new_issuance_asset",
              input_index: 0,
            },
          },
          {
            id: "reissuance_token",
            asset: {
              type: "new_issuance_token",
              input_index: 0,
            },
          },
        ],
      },
    });
  });

  test("builds an actual reissuance request", async () => {
    const envelope = await scenarioToProcessRequestEnvelope(
      createDefaultScenario("reissuance"),
      14,
    );

    expect(envelope.params).toMatchObject({
      network: "testnet-liquid",
      params: {
        inputs: [
          {
            id: "reissuance",
            utxo_source: {
              wallet: {
                filter: {
                  asset: {
                    exact: {
                      asset_id: FIXTURE_REISSUANCE_TOKEN_ASSET,
                    },
                  },
                },
              },
            },
            issuance: {
              kind: "reissue",
              asset_amount_sat: 7,
              token_amount_sat: 0,
              entropy: new Array(32).fill(8),
            },
          },
        ],
        outputs: [
          {
            id: "reissued_asset",
            amount_sat: 7,
            asset: {
              type: "re_issuance_asset",
              input_index: 0,
            },
            blinder: "wallet",
          },
        ],
      },
    });
  });
});
