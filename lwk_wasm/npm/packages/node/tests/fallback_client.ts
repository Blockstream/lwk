import * as lwk from "lwk_node";

export default async function testFallbackClientWithRetry() {
  try {
    const network = lwk.Network.testnet();

    const mnemonic = new lwk.Mnemonic("other august catalog large suit off fan hammer ritual sword evil scrub");
    const signer = new lwk.Signer(mnemonic, network);

    const desc = signer.wpkhSlip77Descriptor();
    const wollet = new lwk.Wollet(network, desc);

    const primary_url = "https://waterfalls.liquidwebwallet.org/liquidtestnet/api";
    const fallback_url = "https://another-waterfalls-server.info/liquidtesnet/api ";
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
