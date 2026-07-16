const lwk = require('lwk_node');
const { fundAddress, waitForTx, WATERFALLS_URL } = require('./scripts/utils');

async function runBasicsTest() {
    try {

        // ANCHOR: generate-signer
        const mnemonic = lwk.Mnemonic.fromRandom(12);
        if (false) { // ANCHOR: ignore
        const network = lwk.Network.testnet();
        } // ANCHOR: ignore
        const network = lwk.Network.regtestDefault(); // ANCHOR: ignore
        const signer = new lwk.Signer(mnemonic, network);
        // ANCHOR_END: generate-signer

        // ANCHOR: get-xpub
        const xpub = signer.keyoriginXpub(lwk.Bip.bip84());
        // ANCHOR_END: get-xpub

        // ANCHOR: wollet
        const desc = signer.wpkhSlip77Descriptor();
        const wollet = new lwk.Wollet(network, desc);
        // ANCHOR_END: wollet

        // ANCHOR: address
        const addr = wollet.address(null).address().toString();
        // ANCHOR_END: address

        // ANCHOR: txs
        const txs = wollet.transactions();
        const balance = wollet.balance();
        // ANCHOR_END: txs

        // TODO: moves example code related to clients.md to a separate file `clients.js`.
        // ANCHOR: esplora_client
        const url_esplora = "https://blockstream.info/liquid/api";
        const esplora_client = new lwk.EsploraClient(lwk.Network.mainnet(), url_esplora, true, 4, false);
        // ANCHOR_END: esplora_client

        // ANCHOR: waterfalls_client
        const url_waterfalls = "https://waterfalls.liquidwebwallet.org/liquid/api";
        const waterfalls_client = new lwk.WaterfallsClient(lwk.Network.mainnet(), url_waterfalls);
        // ANCHOR_END: waterfalls_client

        // ANCHOR: client
        if (false) { // ANCHOR: ignore
        const url = "https://blockstream.info/liquidtestnet/api";
        // TODO: name variables // ANCHOR: ignore
        const client = new lwk.EsploraClient(network, url, true, 4, false);
        } // ANCHOR: ignore
        const client = new lwk.WaterfallsClient(network, WATERFALLS_URL); // ANCHOR: ignore
        let fundTxid = await fundAddress(wollet.address(null).address(), BigInt(100_000), network, client); // ANCHOR: ignore
        await waitForTx(wollet, client, fundTxid); // ANCHOR: ignore

        const update = await client.fullScan(wollet);
        if (update) {
            wollet.applyUpdate(update);
        }
        // ANCHOR_END: client

        // Verify funds exist // ANCHOR: ignore
        const lbtc = network.policyAsset().toString(); // ANCHOR: ignore
        const lbtcBalance = wollet.balance().entries().get(lbtc) || 0n; // ANCHOR: ignore
        console.assert(lbtcBalance > 0n, "Pre-seeded wallet should have L-BTC"); // ANCHOR: ignore

        // ANCHOR: tx
        const sats = BigInt(1000);
        if (false) { // ANCHOR: ignore
        const address = new lwk.Address("<address>");
        const asset = new lwk.AssetId.fromString("<asset>");
        } // ANCHOR: ignore
        const address = wollet.address(null).address(); // ANCHOR: ignore
        const asset = network.policyAsset(); // ANCHOR: ignore

        var builder = new lwk.TxBuilder(network)
        builder = builder.addRecipient(address, sats, asset)
        var pset = builder.finish(wollet)
        // ANCHOR_END: tx

        // ANCHOR: pset-details
        const details = wollet.psetDetails(pset);
        // ANCHOR_END: pset-details

        // ANCHOR: sign
        pset = signer.sign(pset)
        // ANCHOR_END: sign

        // ANCHOR: broadcast
        pset = wollet.finalize(pset)
        const tx = pset.extractTx();
        const txid = await client.broadcastTx(tx)

        // (optional)
        wollet.applyTransaction(tx);
        // ANCHOR_END: broadcast

        console.log(txid.toString());
    } catch (error) {
        console.error("Basics test failed:", error);
        throw error;
    }
}

if (require.main === module) {
    runBasicsTest();
}

module.exports = { runBasicsTest };
