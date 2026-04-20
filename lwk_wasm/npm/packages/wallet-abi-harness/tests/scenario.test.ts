import { describe, expect, test } from "vitest";

import {
  FIXTURE_REISSUANCE_TOKEN_ASSET,
  TESTNET_POLICY_ASSET,
  createDefaultScenario,
  normalizeScenario,
  setScenarioKind,
  setScenarioMode,
  setScenarioNetwork,
} from "../src/scenario.js";

describe("scenario model", () => {
  test("creates stable transfer defaults", () => {
    const scenario = createDefaultScenario("transfer");

    expect(scenario.kind).toBe("transfer");
    expect(scenario.mode).toBe("builder");
    expect(scenario.network).toBe("testnet-liquid");
    expect(scenario.recipient.amountSat).toBe("700");
    expect(scenario.recipient.assetId).toBe(TESTNET_POLICY_ASSET);
  });

  test("creates stable issuance and reissuance defaults", () => {
    const issuance = createDefaultScenario("issuance");
    const reissuance = createDefaultScenario("reissuance");

    expect(issuance.entropySeed).toBe("fixture:issuance");
    expect(reissuance.entropySeed).toBe("fixture:reissuance");
    expect(reissuance.tokenAssetId).toBe(FIXTURE_REISSUANCE_TOKEN_ASSET);
  });

  test("normalizes invalid input back to defaults", () => {
    const scenario = normalizeScenario({
      kind: "split",
      recipients: [{ amountSat: "1" }],
      feeRateSatKvb: "bad",
    });

    expect(scenario.kind).toBe("split");
    if (scenario.kind !== "split") {
      throw new Error("Expected split scenario");
    }
    expect(scenario.recipients).toHaveLength(2);
    expect(scenario.feeRateSatKvb).toBe(1000);
  });

  test("preserves global flags when changing kind and mode", () => {
    const base = setScenarioMode(
      createDefaultScenario("transfer"),
      "walletconnect",
    );
    const next = setScenarioKind(
      setScenarioNetwork(base, "localtest-liquid"),
      "reissuance",
    );

    expect(next.kind).toBe("reissuance");
    expect(next.mode).toBe("walletconnect");
    expect(next.network).toBe("localtest-liquid");
  });
});
