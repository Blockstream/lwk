import * as lwk from "lwk_node";

const network = lwk.Network.testnet();
console.assert(network.toString() === "LiquidTestnet");

console.log("Network test passed!");
