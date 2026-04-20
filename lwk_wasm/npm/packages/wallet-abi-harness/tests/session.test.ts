import type { WalletAbiSessionControllerCallbacks } from "lwk_wallet_abi_sdk";
import type { SessionTypes } from "@walletconnect/types";

import { describe, expect, test } from "vitest";
import {
  Txid,
  WalletAbiTransactionInfo,
  WalletAbiTxCreateResponse,
  networkFromString,
} from "lwk_wallet_abi_sdk";

import { createDefaultScenario } from "../src/scenario.js";
import { scenarioToTxCreateRequest } from "../src/request.js";
import { createHarnessSessionFromController } from "../src/session.js";
import { GREEN_ANDROID_ISSUANCE_ENVELOPE } from "./fixtures/android.js";

function createFakeController() {
  let topic: string | null = "topic-1";
  let callbacks: WalletAbiSessionControllerCallbacks | null = null;
  const requestCalls: Array<{
    method: string;
    params?: unknown;
  }> = [];
  let connectCalls = 0;

  return {
    controller: {
      chainId: "walabi:testnet-liquid" as const,
      session() {
        return topic === null
          ? null
          : ({
              topic,
            } as unknown as SessionTypes.Struct);
      },
      async connect() {
        connectCalls += 1;
        return {
          topic: topic ?? "topic-1",
        } as unknown as SessionTypes.Struct;
      },
      async disconnect() {
        topic = null;
      },
      async request(input: { method: string; params?: unknown }) {
        requestCalls.push(input);
        if (input.method === "get_signer_receive_address") {
          return "tlq1qqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqq3l4q9m";
        }

        if (input.method === "get_raw_signing_x_only_pubkey") {
          return "79be667ef9dcbbac55a06295ce870b07029bfcdb2dce28d959f2815b16f81798";
        }

        const requestId =
          typeof input.params === "object" &&
          input.params !== null &&
          "request_id" in input.params &&
          typeof input.params.request_id === "string"
            ? input.params.request_id
            : "wallet-abi-response";

        if (!/^[0-9a-f-]{36}$/u.test(requestId)) {
          return {
            status: "ok",
          };
        }

        return WalletAbiTxCreateResponse.ok(
          requestId,
          networkFromString("liquid-testnet"),
          WalletAbiTransactionInfo.new(
            "00",
            new Txid(
              "0000000000000000000000000000000000000000000000000000000000000000",
            ),
          ),
        ).toJSON();
      },
      subscribe(nextCallbacks: WalletAbiSessionControllerCallbacks) {
        callbacks = nextCallbacks;
        return () => {
          callbacks = null;
        };
      },
    },
    requestCalls,
    get connectCalls() {
      return connectCalls;
    },
    triggerUpdate() {
      callbacks?.onUpdated?.();
    },
    triggerDisconnect() {
      callbacks?.onDisconnected?.();
    },
  };
}

describe("session wrapper", () => {
  test("logs getter requests and responses", async () => {
    const fake = createFakeController();
    const transcript: Array<{ direction: string; method: string }> = [];
    const session = createHarnessSessionFromController(fake.controller, {
      onTranscript(entry) {
        transcript.push({
          direction: entry.direction,
          method: entry.method,
        });
      },
    });

    const result = await session.getSignerReceiveAddress();

    expect(result.value).toBe("tlq1qqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqq3l4q9m");
    expect(fake.connectCalls).toBe(1);
    expect(fake.requestCalls).toEqual([
      {
        method: "get_signer_receive_address",
        params: {},
      },
    ]);
    expect(transcript).toEqual([
      {
        direction: "outbound",
        method: "get_signer_receive_address",
      },
      {
        direction: "inbound",
        method: "get_signer_receive_address",
      },
    ]);
  });

  test("sends typed process_request payloads", async () => {
    const fake = createFakeController();
    const session = createHarnessSessionFromController(fake.controller, {});
    const request = await scenarioToTxCreateRequest(
      createDefaultScenario("split"),
    );

    await session.processRequest(request);

    expect(fake.requestCalls).toHaveLength(1);
    expect(fake.requestCalls[0]).toMatchObject({
      method: "wallet_abi_process_request",
      params: {
        network: "testnet-liquid",
      },
    });
  });

  test("preserves raw envelope ids on raw transport sends", async () => {
    const fake = createFakeController();
    const session = createHarnessSessionFromController(fake.controller, {});

    const result = await session.sendRawEnvelope(
      GREEN_ANDROID_ISSUANCE_ENVELOPE,
    );

    expect(fake.requestCalls).toEqual([
      {
        method: "wallet_abi_process_request",
        params: GREEN_ANDROID_ISSUANCE_ENVELOPE.params,
      },
    ]);
    expect(result.response).toEqual({
      id: "wallet-abi-issuance-envelope",
      jsonrpc: "2.0",
      result: {
        status: "ok",
      },
    });
  });

  test("forwards lifecycle subscription callbacks", () => {
    const fake = createFakeController();
    const session = createHarnessSessionFromController(fake.controller, {});
    const events: string[] = [];

    const release = session.subscribe({
      onUpdated() {
        events.push("updated");
      },
      onDisconnected() {
        events.push("disconnected");
      },
    });

    fake.triggerUpdate();
    fake.triggerDisconnect();
    release();

    expect(events).toEqual(["updated", "disconnected"]);
  });
});
