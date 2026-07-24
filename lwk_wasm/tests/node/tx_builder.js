const assert = require('assert');
const lwk = require('lwk_node');
const { fundAddress, waitForTx, WATERFALLS_URL } = require('./scripts/utils');

// Issues an asset from `utxo` and returns the new asset UTXO and its L-BTC change UTXO.
async function issueAssetGetUtxos(network, signer, wollet, client, utxo) {
    const request = new lwk.IssuanceRequest(BigInt(1000), BigInt(0));
    let builder = new lwk.TxBuilder(network);
    builder = builder.addIssuance(request);
    builder = builder.setWalletUtxos([utxo.outpoint()]);
    let pset = builder.finish(wollet);
    pset = signer.sign(pset);
    pset = wollet.finalize(pset);
    const txid = await client.broadcastTx(pset.extractTx());
    await waitForTx(wollet, client, txid);

    const policyAsset = network.policyAsset();
    const utxos = wollet.utxos().filter((u) => u.outpoint().txid().toString() === txid.toString());
    const lbtcUtxo = utxos.find((u) => u.unblinded().asset().toString() === policyAsset.toString());
    const assetUtxo = utxos.find((u) => u.unblinded().asset().toString() !== policyAsset.toString());
    return { assetUtxo, lbtcUtxo };
}

async function runManualCoinSelectionTest() {
    try {
        const network = lwk.Network.regtestDefault();
        const client = new lwk.WaterfallsClient(network, WATERFALLS_URL);

        const mnemonic = lwk.Mnemonic.fromRandom(12);
        const signer = new lwk.Signer(mnemonic, network);
        const desc = signer.wpkhSlip77Descriptor();
        const wollet = new lwk.Wollet(network, desc);

        const fundTxid = await fundAddress(wollet.address(0).address(), BigInt(100_000), network, client);
        await waitForTx(wollet, client, fundTxid);

        const utxo = wollet.utxos()[0];
        const addr = wollet.address(null).address();

        // Selecting the wallet's only UTXO covers the send
        let builder = new lwk.TxBuilder(network);
        builder = builder.addRecipient(addr, BigInt(1000), network.policyAsset());
        builder = builder.setWalletUtxos([utxo.outpoint()]);
        const pset = builder.finish(wollet);
        assert.strictEqual(pset.inputs().length, 1);
        assert.strictEqual(pset.outputs().length, 3); // recipient + change + fee
    } catch (error) {
        console.error("Manual coin selection test failed:", error);
        throw error;
    }
}

async function runInputOrderTest() {
    try {
        const network = lwk.Network.regtestDefault();
        const client = new lwk.WaterfallsClient(network, WATERFALLS_URL);
        const policyAsset = network.policyAsset();

        const mnemonic = lwk.Mnemonic.fromRandom(12);
        const signer = new lwk.Signer(mnemonic, network);
        const desc = signer.wpkhSlip77Descriptor();
        const wollet = new lwk.Wollet(network, desc);

        const fundTxid = await fundAddress(wollet.address(0).address(), BigInt(100_000), network, client);
        await waitForTx(wollet, client, fundTxid);

        const { assetUtxo, lbtcUtxo } = await issueAssetGetUtxos(
            network,
            signer,
            wollet,
            client,
            wollet.utxos()[0]
        );
        const addr = wollet.address(null).address();

        let builder = new lwk.TxBuilder(network);
        builder = builder.addRecipient(addr, BigInt(1000), policyAsset);
        builder = builder.setWalletUtxos([assetUtxo.outpoint(), lbtcUtxo.outpoint()]);
        builder = builder.setInputsOrder([assetUtxo.outpoint(), lbtcUtxo.outpoint()]);
        const pset = builder.finish(wollet);
        const inputs = pset.inputs();
        assert.strictEqual(inputs.length, 2);
        assert.strictEqual(inputs[0].previousTxid().toString(), assetUtxo.outpoint().txid().toString());
        assert.strictEqual(inputs[0].previousVout(), assetUtxo.outpoint().vout());
        assert.strictEqual(inputs[1].previousTxid().toString(), lbtcUtxo.outpoint().txid().toString());
        assert.strictEqual(inputs[1].previousVout(), lbtcUtxo.outpoint().vout());
    } catch (error) {
        console.error("Input order test failed:", error);
        throw error;
    }
}

async function runIssueAssetTest() {
    try {
        const network = lwk.Network.regtestDefault();
        const client = new lwk.WaterfallsClient(network, WATERFALLS_URL);

        const mnemonic = lwk.Mnemonic.fromRandom(12);
        const signer = new lwk.Signer(mnemonic, network);
        const desc = signer.wpkhSlip77Descriptor();
        const wollet = new lwk.Wollet(network, desc);

        const fundTxid0 = await fundAddress(wollet.address(0).address(), BigInt(100_000), network, client);
        await waitForTx(wollet, client, fundTxid0);
        const fundTxid1 = await fundAddress(wollet.address(1).address(), BigInt(100_000), network, client);
        await waitForTx(wollet, client, fundTxid1);
        const fundTxid2 = await fundAddress(wollet.address(2).address(), BigInt(100_000), network, client);
        await waitForTx(wollet, client, fundTxid2);

        const utxos = wollet.utxos();
        assert.strictEqual(utxos.length, 3);

        // Two issuances in the same transaction, sequentially assigned to two of the wallet's UTXOs
        const request1 = new lwk.IssuanceRequest(BigInt(1000), BigInt(1));
        const request2 = new lwk.IssuanceRequest(BigInt(2000), BigInt(2));
        let builder = new lwk.TxBuilder(network);
        builder = builder.addIssuance(request1);
        builder = builder.addIssuance(request2);
        builder = builder.setWalletUtxos([utxos[0].outpoint(), utxos[1].outpoint()]);
        const pset = builder.finish(wollet);

        const inputs = pset.inputs();
        assert.strictEqual(inputs.length, 2);

        const issuanceInputs = inputs.filter((i) => i.issuance() !== undefined);
        assert.strictEqual(issuanceInputs.length, 2);
        for (const input of issuanceInputs) {
            assert.strictEqual(input.issuance().isIssuance(), true);
            assert.strictEqual(input.issuance().isReissuance(), false);
            assert.notStrictEqual(input.issuanceAsset(), undefined);
            assert.notStrictEqual(input.issuanceToken(), undefined);
        }
        assert.notStrictEqual(
            issuanceInputs[0].issuanceAsset().toString(),
            issuanceInputs[1].issuanceAsset().toString()
        );

        // Two issuances in the same transaction, each pinned to a different input: a fresh
        // asset UTXO (issued from the wallet's third UTXO) and its L-BTC change
        const { assetUtxo, lbtcUtxo } = await issueAssetGetUtxos(network, signer, wollet, client, utxos[2]);

        const request3 = new lwk.IssuanceRequest(BigInt(3000), BigInt(5));
        const request4 = new lwk.IssuanceRequest(BigInt(4000), BigInt(6));
        let pinnedBuilder = new lwk.TxBuilder(network);
        pinnedBuilder = pinnedBuilder.setWalletUtxos([assetUtxo.outpoint(), lbtcUtxo.outpoint()]);
        pinnedBuilder = pinnedBuilder.setInputsOrder([assetUtxo.outpoint(), lbtcUtxo.outpoint()]);
        pinnedBuilder = pinnedBuilder.addIssuance(request3.pinInput(assetUtxo.outpoint()));
        pinnedBuilder = pinnedBuilder.addIssuance(request4.pinInput(lbtcUtxo.outpoint()));
        const pinnedPset = pinnedBuilder.finish(wollet);

        const pinnedInputs = pinnedPset.inputs();
        assert.strictEqual(pinnedInputs.length, 2);

        assert.strictEqual(pinnedInputs[0].previousTxid().toString(), assetUtxo.outpoint().txid().toString());
        assert.strictEqual(pinnedInputs[0].previousVout(), assetUtxo.outpoint().vout());
        assert.notStrictEqual(pinnedInputs[0].issuance(), undefined);
        assert.strictEqual(pinnedInputs[0].issuance().isIssuance(), true);

        assert.strictEqual(pinnedInputs[1].previousTxid().toString(), lbtcUtxo.outpoint().txid().toString());
        assert.strictEqual(pinnedInputs[1].previousVout(), lbtcUtxo.outpoint().vout());
        assert.notStrictEqual(pinnedInputs[1].issuance(), undefined);
        assert.strictEqual(pinnedInputs[1].issuance().isIssuance(), true);

        assert.notStrictEqual(
            pinnedInputs[0].issuanceAsset().toString(),
            pinnedInputs[1].issuanceAsset().toString()
        );
    } catch (error) {
        console.error("Issue asset test failed:", error);
        throw error;
    }
}

async function runTxBuilderTest() {
    await runManualCoinSelectionTest();
    await runInputOrderTest();
    await runIssueAssetTest();
}

if (require.main === module) {
    runTxBuilderTest().then(() => {
        console.log("tx_builder.js: all tests passed");
    });
}

module.exports = { runTxBuilderTest };
