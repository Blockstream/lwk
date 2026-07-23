const lwk = require('lwk_node');
const { fundAddress, waitForTx, generateAddress, WATERFALLS_URL } = require('./scripts/utils');

async function runDrainLbtcTest() {
  try {
    const network = lwk.Network.regtestDefault();
    const client = new lwk.WaterfallsClient(network, WATERFALLS_URL);

    // Create wallet
    const mnemonic = lwk.Mnemonic.fromRandom(12);

    const signer = new lwk.Signer(mnemonic, network);
    const desc = signer.wpkhSlip77Descriptor();

    const wollet = new lwk.Wollet(network, desc);

    const fundedSatoshi = 100000;
    var fundTxid = await fundAddress(wollet.address(0).address(), BigInt(fundedSatoshi), network, client);
    await waitForTx(wollet, client, fundTxid);

    const node_addr = generateAddress();

    // ANCHOR: drain_lbtc_wallet
    // Create a PSET sending all LBTC to the node address
    var builder = new lwk.TxBuilder(network);
    builder = builder.drainLbtcWallet();
    builder = builder.drainLbtcTo(node_addr);
    var pset = builder.finish(wollet);
    // ANCHOR_END: drain_lbtc_wallet

    var signedPset = signer.sign(pset);
    var finalizedPset = wollet.finalize(signedPset);
    var tx = finalizedPset.extractTx();
    var txid = await client.broadcastTx(tx);
    await waitForTx(wollet, client, txid);

    const policy_asset = network.policyAsset().toString();
    const lbtcBalance = wollet.balance().entries().get(policy_asset) || 0n;
    if (lbtcBalance !== 0n) throw new Error("Expected LBTC balance to be 0");
    if (wollet.transactions().length !== 2) throw new Error("Expected 2 transactions");

  } catch (error) {
    console.error("Drain LBTC test failed:", error);
    throw error;
  }
}

if (require.main === module) {
  runDrainLbtcTest();
}

module.exports = { runDrainLbtcTest };
