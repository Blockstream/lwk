import { describe, expect, test } from "vitest";

import { createDefaultScenario, setScenarioMode } from "../src/scenario.js";
import {
  createShareUrl,
  decodeHarnessLocation,
  encodeRawEnvelopeHash,
  encodeScenarioHash,
} from "../src/url.js";

describe("url codec", () => {
  test("round-trips versioned scenarios", () => {
    const scenario = setScenarioMode(
      createDefaultScenario("split"),
      "walletconnect",
    );
    const decoded = decodeHarnessLocation(`#${encodeScenarioHash(scenario)}`);

    expect(decoded.rawEnvelope).toBeNull();
    expect(decoded.scenario).toEqual(scenario);
  });

  test("round-trips raw envelopes", () => {
    const decoded = decodeHarnessLocation(
      `#${encodeRawEnvelopeHash({
        jsonrpc: "2.0",
        id: "raw-envelope",
        method: "wallet_abi_process_request",
        params: {
          ok: true,
        },
      })}`,
    );

    expect(decoded.rawEnvelope).toEqual({
      jsonrpc: "2.0",
      id: "raw-envelope",
      method: "wallet_abi_process_request",
      params: {
        ok: true,
      },
    });
  });

  test("falls back to defaults on malformed hashes", () => {
    const decoded = decodeHarnessLocation("#scenario=%%%");

    expect(decoded.scenario.kind).toBe("transfer");
    expect(decoded.rawEnvelope).toBeNull();
  });

  test("prefers raw share URLs when a raw envelope is provided", () => {
    const url = createShareUrl("https://example.com/harness", {
      scenario: createDefaultScenario("transfer"),
      rawEnvelope: {
        jsonrpc: "2.0",
        id: 1,
        method: "get_signer_receive_address",
      },
    });

    expect(url).toContain("#raw=");
  });
});
