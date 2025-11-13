async function runAmp0DailyOps() {
    try {
        const ciBranchName = process.env.CI_COMMIT_BRANCH;
        if (ciBranchName !== undefined) {
            // We are in a CI job
            if (ciBranchName !== "master") {
                console.log("Skipping test");
                process.exit(0);
            }
        }

        const WebSocket = require('ws');
        global.WebSocket = WebSocket;

        const lwk = require('lwk_node');

        // ANCHOR: amp0-daily-ops
	if (true) { // ANCHOR: ignore
        const mnemonic = "<mnemonic>";
        } // ANCHOR: ignore
        const mnemonic = "thrive metal cactus come oval candy medal bounce captain shock permit joke"; // ANCHOR: ignore
        const m = new lwk.Mnemonic(mnemonic);
        const network = lwk.Network.testnet();
        const signer = new lwk.Signer(m, network);
	if (true) { // ANCHOR: ignore
        const username = "<username>";
        const password = "<password>";
        } // ANCHOR: ignore
        const username = "userlwk001"; // ANCHOR: ignore
        const password = "userlwk001"; // ANCHOR: ignore
        const amp_id = "";

        // Create AMP0 object
        const amp0 = await lwk.Amp0.newTestnet(username, password, amp_id);

        // Get an address
        const addrResult = await amp0.address(1);

        // Create wollet
        const wollet = amp0.wollet();

        // Sync the wallet
        const url = "https://waterfalls.liquidwebwallet.org/liquidtestnet/api";
        const client = new lwk.EsploraClient(network, url, true, 4, false);

        const last_index = amp0.lastIndex();
        const update = await client.fullScanToIndex(wollet, last_index);
        if (update) {
            wollet.applyUpdate(update);
        }

        // Get the wallet transactions
        const txs = wollet.transactions();

        // Get the balance
        const balance = wollet.balance();
        const lbtc = network.policyAsset(); // ANCHOR: ignore
        const lbtc_balance = balance.entries().get(lbtc.toString()) || 0n; // ANCHOR: ignore
        if (lbtc_balance < 500n) { // ANCHOR: ignore
            const addr1 = addrResult.address().toString(); // ANCHOR: ignore
            console.log(`Balance is insufficient to make a transaction, send some tLBTC to ${addr1}`); // ANCHOR: ignore
            return; // ANCHOR: ignore
        } // ANCHOR: ignore

        // Create a (redeposit) transaction
        var b = network.txBuilder();
        b = b.drainLbtcWallet();
        const amp0pset = b.finishForAmp0(wollet);

        // Sign with the user key
        const pset = amp0pset.pset();
        const signed_pset = signer.sign(pset);

        // Ask AMP0 to cosign
        const amp0pset_signed = new lwk.Amp0Pset(signed_pset, amp0pset.blindingNonces());
        const tx = await amp0.sign(amp0pset_signed);

        // Broadcast
        const txid = await client.broadcastTx(tx);
        // ANCHOR_END: amp0-daily-ops
        console.log(txid.toString());
        process.exit(0); // TODO: expose a way to close the websocket
    } catch (error) {
        console.error("AMP0 test failed:", error);
        process.exit(1);
    }
}

if (require.main === module) {
    runAmp0DailyOps();
}

module.exports = { runAmp0DailyOps };
