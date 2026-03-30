import * as assert from "node:assert/strict";
import * as lwk from "@blockstream/lwk-node";

export async function runNetworkTest(): Promise<void> {
  const network = lwk.Network.testnet();
  assert.equal(network.toString(), "LiquidTestnet");

  console.log("Network test passed!");
}
