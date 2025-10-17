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

	const lbtc = network.policyAsset().toString();
	const lbtc_balance = wollet_c.balance().entries().get(lbtc) || 0n;
	if (lbtc_balance < 500n) {
	    console.log(`Balance is insufficient to make a transaction, send some tLBTC to ${addr}`);
	    return;
	}

	// ANCHOR: multisig-send
	// Carol creates a transaction send few sats to a certain address
	const sats = BigInt(1000);
	if (false) { // ANCHOR: ignore
	const address = new lwk.Address("<address>");
	const asset = new lwk.AssetId("<asset>");
	} // ANCHOR: ignore
	const address = new lwk.Address("tlq1qq2g07nju42l0nlx0erqa3wsel2l8prnq96rlnhml262mcj7pe8w6ndvvyg237japt83z24m8gu4v3yfhaqvrqxydadc9scsmw"); // ANCHOR: ignore
	const asset = network.policyAsset(); // ANCHOR: ignore

	var builder = new lwk.TxBuilder(network)
	builder = builder.addRecipient(address, sats, asset)
	var pset = builder.finish(wollet_c)

	pset = signer_c.sign(pset)

	// Carol sends the PSET to Bob
	// Bob wants to analyze the PSET before signing, thus he creates a wollet
	const wd_b = new lwk.WolletDescriptor(desc);
	const wollet_b = new lwk.Wollet(network, wd_b);
	const update_b = await client.fullScan(wollet_b);
	if (update_b) {
	    wollet_b.applyUpdate(update_b);
	}
	// Then Bob uses the wollet to analyze the PSET
	const details = wollet_b.psetDetails(pset);
	// PSET has a reasonable fee
	console.assert(details.balance().fee() < 100);
	// PSET has a signature from Carol
	console.assert(details.fingerprintsHas().length === 1);
	const fingerprint_c = xpub_c.substring(1, 9);
	console.assert(details.fingerprintsHas().includes(fingerprint_c));
	// PSET needs a signature from either Bob or Carol
	console.assert(details.fingerprintsMissing().length === 2);
	const fingerprint_a = xpub_a.substring(1, 9);
	const fingerprint_b = xpub_b.substring(1, 9);
	console.assert(details.fingerprintsMissing().includes(fingerprint_a));
	console.assert(details.fingerprintsMissing().includes(fingerprint_b));
	// PSET has a single recipient, with data matching what was specified above
	console.assert(details.balance().recipients().length === 1);
	const recipient = details.balance().recipients()[0];
	console.assert(recipient.address().toString() === address.toString());
	console.assert(recipient.asset().toString() === asset.toString());
	console.assert(recipient.value() === sats);

	// Bob is satisified with the PSET and signs it
	pset = signer_b.sign(pset)

	// Bob sends the PSET back to Carol
	// Carol checks that the PSET has enough signatures
	const details_b = wollet_b.psetDetails(pset);
	console.assert(details_b.fingerprintsHas().length === 2);

	// Carol finalizes the PSET and broadcast the transaction
	pset = wollet_c.finalize(pset)
	const tx = pset.extractTx();
	const txid = await client.broadcastTx(tx);
	// ANCHOR_END: multisig-send
	console.log(txid.toString());
    } catch (error) {
	console.error("Basics test failed:", error);
	throw error;
    }
}

if (require.main === module) {
    runMultisigTest();
}

module.exports = { runMultisigTest };
