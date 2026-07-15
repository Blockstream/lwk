const lwk = require('lwk_node');
const { WATERFALLS_URL } = require("./scripts/utils.js");

async function testFallbackClientWithRetry() {
    try {
        const network = lwk.Network.regtestDefault();

        const mnemonic = lwk.Mnemonic.fromRandom(12);
        const signer = new lwk.Signer(mnemonic, network);

        const desc = signer.wpkhSlip77Descriptor();
        const wollet = new lwk.Wollet(network, desc);

        const primary_url = "https://primary-waterfalls-server.info/liquidtesnet/api";
        const fallback_url = WATERFALLS_URL;
        const waterfalls = true;
        const concurrency = 4;
        const utxo_only = false;

        // ANCHOR: fallback_client
        const client = new lwk.EsploraClient(network, primary_url, waterfalls, concurrency, utxo_only);

        let update;

        try {
            update = await client.fullScan(wollet);
        } catch (error) {
            // Falling into a retryable error, making a request with the fallback client
            const fallbackClient = new lwk.EsploraClient(network, fallback_url, waterfalls, concurrency, utxo_only);
            update = await fallbackClient.fullScan(wollet);
        }

        if (update) {
            wollet.applyUpdate(update);
        }
        // ANCHOR_END: fallback_client

        console.log("Fallback client test passed!");
        return wollet;

    } catch (error) {
        console.error("Fallback client test failed:", error);
        throw error;
    }
}

if (require.main === module) {
    testFallbackClientWithRetry();
}

module.exports = {testFallbackClientWithRetry};
