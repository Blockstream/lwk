import * as lwk from "lwk_node";

export default function runNetworkTest() {
  const network = lwk.Network.testnet();
  console.assert(network.toString() === "LiquidTestnet");

  console.log("Network test passed!");
}