const WebSocket = require('ws');
global.WebSocket = WebSocket;

const lwk = require('lwk_node');

console.log("Starting AMP0 test");

// AMP0 credentials
const username = "userlwk001";
const password = "userlwk001";
// AMP ID
const amp_id = "";

// BIP39 mnemonic corresponding to the AMP0 account
const mnemonic = new lwk.Mnemonic("thrive metal cactus come oval candy medal bounce captain shock permit joke");

const network = lwk.Network.testnet();
const url = "https://waterfalls.liquidwebwallet.org/liquidtestnet/api";

// Create AMP0 object
async function runAmp0Test() {
    try {
        console.log("Connecting to AMP0 server");
        const amp0 = await lwk.Amp0.newTestnet(username, password, amp_id);
        console.log("Successfully connected to AMP0 server");

        // Get an address
        const addrResult = await amp0.address(1);
        const addr1 = addrResult.address().toString();
        console.assert(addr1 === "vjTxG9xqzTD2axryND7TjTj7YqitakaDfvUaqrnSuyp3XScTezBiieU9ZrBfoHATxe3xUTt1uzwNAJo5", "Address mismatch");

        // Create wollet
        const wollet = amp0.wollet();

        console.log("Successfully created wollet");

        // Sync the wallet
        const client = new lwk.EsploraClient(network, url, true, 4, false);

        console.log("Successfully created esplora client");

        const last_index = amp0.lastIndex();
        console.assert(last_index > 20, "Last index should be greater than 20");
        const update = await client.fullScanToIndex(wollet, last_index);
        if (update) {
            wollet.applyUpdate(update);
        }

        console.log("Successfully synced wallet");

        // Get the wallet transactions
        const txs = wollet.transactions();
        console.assert(txs.length > 0, "Should have transactions");

        // Get the balance
        const balance = wollet.balance();
        console.log("balance:", balance.entries());

        const lbtc = network.policyAsset();
        const lbtc_balance = balance.entries().get(lbtc.toString()) || 0n;
        console.log("lbtc_balance:", lbtc_balance);
        if (lbtc_balance < 500n) {
            console.log(`Balance is insufficient to make a transaction, send some tLBTC to ${addr1}`);
            return;
        }

        console.log("Successfully got wallet balance");

        // Create a (redeposit) transaction
        var b = network.txBuilder();
        b = b.drainLbtcWallet();
        const amp0pset = b.finishForAmp0(wollet);

        // Create the signer
        const signer = new lwk.Signer(mnemonic, network);

        // Sign with the user key
        const pset = amp0pset.pset();
        const signed_pset = signer.sign(pset);

        // Ask AMP0 to cosign
        const amp0pset_signed = new lwk.Amp0Pset(signed_pset, amp0pset.blindingNonces());
        const tx = await amp0.sign(amp0pset_signed);

        // Broadcast
        const txid = await client.broadcastTx(tx);
        console.log(`Transaction broadcasted with txid: ${txid.toString()}`);

        console.log("AMP0 test passed!");
        process.exit(0); // TODO: expose a way to close the websocket
    } catch (error) {
        console.error("AMP0 test failed:", error);
        process.exit(1);
    }
}

if (require.main === module) {
    runAmp0Test();
}

module.exports = { runAmp0Test };
