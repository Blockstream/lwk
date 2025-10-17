const lwk = require('lwk_node');

// TODO: use regtest instead of testnet:
// however keep displaying testnet in the generated docs using // ANCHOR: ignore

async function runMultisigTest() {
    try {
	// ANCHOR: multisig-setup
	const network = lwk.Network.testnet();
	// Derivation for multisig
	const bip = lwk.Bip.bip87();

	// Alice creates their signer and gets the xpub
	if (false) { // ANCHOR: ignore
	const mnemonic_a = lwk.Mnemonic.fromRandom(12);
	} // ANCHOR: ignore
	// Fixed mnemonic with some funds // ANCHOR: ignore
	const mnemonic_a = new lwk.Mnemonic("kind they sing appear whip boil divorce essence mask alien teach wire"); // ANCHOR: ignore
	const signer_a = new lwk.Signer(mnemonic_a, network);
	const xpub_a = signer_a.keyoriginXpub(bip);

	// Bob creates their signer and gets the xpub
	if (false) { // ANCHOR: ignore
	const mnemonic_b = lwk.Mnemonic.fromRandom(12);
	} // ANCHOR: ignore
	// Fixed mnemonic with some funds // ANCHOR: ignore
	const mnemonic_b = new lwk.Mnemonic("vast response truth other mansion skull hold amused capital satoshi oxygen brass"); // ANCHOR: ignore
	const signer_b = new lwk.Signer(mnemonic_b, network);
	const xpub_b = signer_b.keyoriginXpub(bip);

	// Carol, who acts as a coordinator, creates their signer and gets the xpub
	if (false) { // ANCHOR: ignore
	const mnemonic_c = lwk.Mnemonic.fromRandom(12);
	} // ANCHOR: ignore
	// Fixed mnemonic with some funds // ANCHOR: ignore
	const mnemonic_c = new lwk.Mnemonic("fresh inner begin grid symbol congress wall outer mass enable coil repeat"); // ANCHOR: ignore
	const signer_c = new lwk.Signer(mnemonic_c, network);
	const xpub_c = signer_c.keyoriginXpub(bip);

	// Carol generates a random SLIP77 descriptor blinding key
	if (false) { // ANCHOR: ignore
	const slip77_rand_key = "<random-64-hex-chars>";
	} // ANCHOR: ignore
	const slip77_rand_key = "1111111111111111111111111111111111111111111111111111111111111111"; // ANCHOR: ignore
	const desc_blinding_key = `slip77(${slip77_rand_key})`;

	// Carol uses the collected xpubs and the descriptor blinding key to create
	// the 2of3 descriptor
	const threshold = 2;
	const desc = `ct(${desc_blinding_key},elwsh(multi(${threshold},${xpub_a}/<0;1>/*,${xpub_b}/<0;1>/*,${xpub_c}/<0;1>/*)))`;
	// Validate the descriptor string
	const wd = new lwk.WolletDescriptor(desc);
	// ANCHOR_END: multisig-setup

	// ANCHOR: multisig-receive
	// Carol creates the wollet
	const wollet_c = new lwk.Wollet(network, wd);

	// With the wollet, Carol can obtain addresses, transactions and balance
	const addr = wollet_c.address(null).address().toString();
	const txs = wollet_c.transactions();
	const balance = wollet_c.balance();

	// Update the wollet state
	const url = "https://waterfalls.liquidwebwallet.org/liquidtestnet/api";
	// TODO: name variables // ANCHOR: ignore
	const client = new lwk.EsploraClient(network, url, true, 4, false);

	const update = await client.fullScan(wollet_c);
	if (update) {
	    wollet_c.applyUpdate(update);
	}
	// ANCHOR_END: multisig-receive
    } catch (error) {
	console.error("Basics test failed:", error);
	throw error;
    }
}

if (require.main === module) {
    runMultisigTest();
}

module.exports = { runMultisigTest };
