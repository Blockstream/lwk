const assert = require('node:assert/strict');
const lwk = require('lwk_node');

const network = lwk.Network.testnet();
assert.equal(network.toString(), "LiquidTestnet");

console.log("Network test passed!");
