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

async function runReissueAssetTest() {
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

        const utxos = wollet.utxos();
        assert.strictEqual(utxos.length, 2);

        // Issue two assets in the same transaction, each with a reissuance token, and
        // broadcast so the wallet actually owns the tokens needed to reissue them
        const issuanceRequest0 = new lwk.IssuanceRequest(BigInt(1000), BigInt(1));
        const issuanceRequest1 = new lwk.IssuanceRequest(BigInt(2000), BigInt(1));
        let builder = new lwk.TxBuilder(network);
        builder = builder.addIssuance(issuanceRequest0);
        builder = builder.addIssuance(issuanceRequest1);
        builder = builder.setWalletUtxos([utxos[0].outpoint(), utxos[1].outpoint()]);
        let pset = builder.finish(wollet);

        const issuanceInputs = pset.inputs().filter((i) => i.issuance() !== undefined);
        const asset0 = issuanceInputs[0].issuanceAsset();
        const asset1 = issuanceInputs[1].issuanceAsset();

        pset = signer.sign(pset);
        pset = wollet.finalize(pset);
        const issuanceTxid = await client.broadcastTx(pset.extractTx());
        await waitForTx(wollet, client, issuanceTxid);

        // Reissue both assets in the same transaction
        const reissuanceRequest0 = new lwk.ReissuanceRequest(asset0, BigInt(500));
        const reissuanceRequest1 = new lwk.ReissuanceRequest(asset1, BigInt(700));
        let reissuanceBuilder = new lwk.TxBuilder(network);
        reissuanceBuilder = reissuanceBuilder.addReissuance(reissuanceRequest0);
        reissuanceBuilder = reissuanceBuilder.addReissuance(reissuanceRequest1);
        const reissuancePset = reissuanceBuilder.finish(wollet);

        const reissuanceInputs = reissuancePset.inputs().filter((i) => i.issuance() !== undefined);
        assert.strictEqual(reissuanceInputs.length, 2);
        for (const input of reissuanceInputs) {
            assert.strictEqual(input.issuance().isIssuance(), false);
            assert.strictEqual(input.issuance().isReissuance(), true);
        }
        const reissuedAssets = reissuanceInputs.map((i) => i.issuanceAsset().toString()).sort();
        const expectedAssets = [asset0.toString(), asset1.toString()].sort();
        assert.deepStrictEqual(reissuedAssets, expectedAssets);
    } catch (error) {
        console.error("Reissue asset test failed:", error);
        throw error;
    }
}

async function runIssuanceOutputsTest() {
    try {
        const network = lwk.Network.regtestDefault();
        const client = new lwk.WaterfallsClient(network, WATERFALLS_URL);

        const mnemonic = lwk.Mnemonic.fromRandom(12);
        const signer = new lwk.Signer(mnemonic, network);
        const desc = signer.wpkhSlip77Descriptor();
        const wollet = new lwk.Wollet(network, desc);

        const fundTxid = await fundAddress(wollet.address(0).address(), BigInt(100_000), network, client);
        await waitForTx(wollet, client, fundTxid);

        // Issue 2 asset units, split across two outputs of 1 unit each
        const request = new lwk.IssuanceRequest(BigInt(2), BigInt(1))
            .addAssetOutput(BigInt(1), null)
            .addAssetOutput(BigInt(1), null);
        let builder = new lwk.TxBuilder(network);
        builder = builder.addIssuance(request);
        let pset = builder.finish(wollet);

        const issuanceInput = pset.inputs().filter((i) => i.issuance() !== undefined)[0];
        const asset = issuanceInput.issuanceAsset();

        pset = signer.sign(pset);
        pset = wollet.finalize(pset);
        const txid = await client.broadcastTx(pset.extractTx());
        await waitForTx(wollet, client, txid);

        const assetOutputs = wollet.utxos().filter((u) =>
            u.outpoint().txid().toString() === txid.toString() &&
            u.unblinded().asset().toString() === asset.toString()
        );
        assert.strictEqual(assetOutputs.length, 2);
        for (const output of assetOutputs) {
            assert.strictEqual(output.unblinded().value(), BigInt(1));
        }
        // Outputs without an explicit address get a fresh address each
        assert.notStrictEqual(
            assetOutputs[0].scriptPubkey().toString(),
            assetOutputs[1].scriptPubkey().toString()
        );
    } catch (error) {
        console.error("Issuance outputs test failed:", error);
        throw error;
    }
}

async function runTxBuilderTest() {
    await runManualCoinSelectionTest();
    await runInputOrderTest();
    await runIssueAssetTest();
    await runReissueAssetTest();
    await runIssuanceOutputsTest();
}

if (require.main === module) {
    runTxBuilderTest().then(() => {
        console.log("tx_builder.js: all tests passed");
    });
}

module.exports = { runTxBuilderTest };
