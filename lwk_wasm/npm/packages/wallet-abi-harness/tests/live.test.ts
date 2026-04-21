import { describe, expect, test } from "vitest";

import { createDefaultScenario, setScenarioMode } from "../src/scenario.js";
import { createShareUrl } from "../src/url.js";

const liveEnabled = process.env.WALLET_ABI_HARNESS_LIVE === "1";

describe.runIf(liveEnabled)("manual live smoke scaffolding", () => {
  test("emits walletconnect URLs for transfer and split live flows", () => {
    const baseUrl = "http://127.0.0.1:4178";
    const urls = [
      createShareUrl(baseUrl, {
        scenario: setScenarioMode(
          createDefaultScenario("transfer"),
          "walletconnect",
        ),
      }),
      createShareUrl(baseUrl, {
        scenario: setScenarioMode(
          createDefaultScenario("split"),
          "walletconnect",
        ),
      }),
    ];

    expect(urls).toHaveLength(2);
    for (const url of urls) {
      expect(url).toContain("#scenario=");
    }
  });
});
