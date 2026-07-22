const lwk = require('lwk_node');
const { fundAddress, waitForTx, generateAddress, WATERFALLS_URL } = require('./scripts/utils');

async function runManualCoinSelectionTest() {
  try {
    const network = lwk.Network.regtestDefault();
    const client = new lwk.WaterfallsClient(network, WATERFALLS_URL);

    // Create wallet
    const mnemonic = lwk.Mnemonic.fromRandom(12);

    const signer = new lwk.Signer(mnemonic, network);
    const desc = signer.wpkhSlip77Descriptor();

    const wollet = new lwk.Wollet(network, desc);

    // Fund wallet with 2 utxos
    const fundedSatoshi = 100000;
    var fundTxid = await fundAddress(wollet.address(0).address(), BigInt(fundedSatoshi), network, client);
    await waitForTx(wollet, client, fundTxid);
    var fundTxid = await fundAddress(wollet.address(1).address(), BigInt(fundedSatoshi), network, client);
    await waitForTx(wollet, client, fundTxid);

    const sent_satoshi = 1000
    const node_address = generateAddress();
    // ANCHOR: get_utxos
    const utxos = wollet.utxos();
    // ANCHOR_END: get_utxos

    // ANCHOR: manual_coin_selection
    var builder = new lwk.TxBuilder(network);
    builder = builder.addLbtcRecipient(node_address, BigInt(sent_satoshi))
    builder = builder.setWalletUtxos([utxos[0].outpoint()])
    var unsignedPset = builder.finish(wollet);

    console.assert(unsignedPset.inputs().length === 1); // ANCHOR: ignore

    var signedPset = signer.sign(unsignedPset);
    var finalizedPset = wollet.finalize(signedPset);
    var tx = finalizedPset.extractTx();
    // ANCHOR_END: manual_coin_selection
    var txid = await client.broadcastTx(tx);
    await waitForTx(wollet, client, txid);

  } catch (error) {
    console.error("Manual coin selection test failed:", error);
    throw error;
  }
}

if (require.main === module) {
  runManualCoinSelectionTest();
}

module.exports = { runManualCoinSelectionTest };
