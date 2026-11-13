const assert = require('node:assert/strict');
const lwk = require('lwk_node');

const viewKey = "3e129856c574c66d94023ac98b7f69aca9774d10aee4dc087f0c52a498687189";
const xpub = "tpubDC8msFGeGuwnKG9Upg7DM2b4DaRqg3CUZa5g8v2SRQ6K4NSkxUgd7HsL2XVWbVm39yBA4LAxysQAm397zwQSQoQgewGiYZqrA9DsP4zbQ1M";
const network = lwk.Network.testnet();

let desc = new lwk.WolletDescriptor(`ct(${viewKey},elwpkh([73c5da0a/84h/1h/0h]${xpub}/<0;1>/*))`);
let wollet = new lwk.Wollet(network, desc);

// The full derivation path of an address, e.g. to sign a message with its key
assert.equal(wollet.address(3).derivationPath().toString(), "84'/1'/0'/0/3");
assert.equal(wollet.change(3).derivationPath().toString(), "84'/1'/0'/1/3");

// Without key origin the path from the master key is unknown
desc = new lwk.WolletDescriptor(`ct(${viewKey},elwpkh(${xpub}/<0;1>/*))`);
wollet = new lwk.Wollet(network, desc);
assert.equal(wollet.address(3).derivationPath(), undefined);

console.log("Address test passed!");
