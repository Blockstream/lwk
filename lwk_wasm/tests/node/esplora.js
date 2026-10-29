const assert = require('node:assert/strict');
const lwk = require('lwk_node');

function runEsploraTest() {
  try {
    const network = lwk.Network.regtestDefault();

    const esploraUrl = "https://esplora.example.com/api";
    const client = new lwk.EsploraClientBuilder(network, esploraUrl)
      .concurrency(4)
      .scanTimeout(60)
      .maxResponseSize(1024 * 1024)
      .timeout(30)
      .header("X-Custom-Header", "value")
      .build();
    assert(client instanceof lwk.EsploraClient);

    assert.throws(() => new lwk.EsploraClientBuilder(network, esploraUrl).timeout(256));
    const maxTimeoutClient = new lwk.EsploraClientBuilder(network, esploraUrl)
      .timeout(255)
      .build();
    assert(maxTimeoutClient instanceof lwk.EsploraClient);

    const waterfallsUrl = "https://waterfalls.example.com/api";
    assert.throws(() => new lwk.WaterfallsClientBuilder(network, waterfallsUrl).timeout(256));
    const maxTimeoutWaterfallsClient = new lwk.WaterfallsClientBuilder(network, waterfallsUrl)
      .timeout(255)
      .build();
    assert(maxTimeoutWaterfallsClient instanceof lwk.WaterfallsClient);

    console.log("Esplora test passed!");
  } catch (error) {
    console.error("Esplora test failed:", error);
    throw error;
  }
}

if (require.main === module) {
  runEsploraTest();
}

module.exports = { runEsploraTest };
