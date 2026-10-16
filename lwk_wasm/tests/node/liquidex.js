const assert = require('assert');
const lwk = require('lwk_node');
const { fundAddress, waitForTx, WATERFALLS_URL } = require('./scripts/utils');

async function runLiquidexTest() {
    try {
        const network = lwk.Network.regtestDefault();
        const client = new lwk.WaterfallsClient(network, WATERFALLS_URL);
        const policyAsset = network.policyAsset();
        const utxoSize = 100_000n;
        const issuedAssetUnits = 100n;

        // (maker) Create a new wallet with LBTC
        const makerSigner = new lwk.Signer(lwk.Mnemonic.fromRandom(12), network);
        const maker = new lwk.Wollet(network, makerSigner.wpkhSlip77Descriptor());
        const makerFundTxid = await fundAddress(maker.address(0).address(), utxoSize, network, client);
        await waitForTx(maker, client, makerFundTxid);
        assert.strictEqual(maker.balance().entries().get(policyAsset.toString()), utxoSize);

        // (taker) Create a second wallet with an issued asset and some LBTC for the fee
        const takerSigner = new lwk.Signer(lwk.Mnemonic.fromRandom(12), network);
        const taker = new lwk.Wollet(network, takerSigner.wpkhSlip77Descriptor());
        const takerFundTxid = await fundAddress(taker.address(0).address(), 10_000n, network, client);
        await waitForTx(taker, client, takerFundTxid);

        const request = new lwk.IssuanceRequest(issuedAssetUnits, 0n);
        let issuePset = new lwk.TxBuilder(network).addIssuance(request).finish(taker);
        const asset = issuePset.inputs().find((i) => i.issuance() !== undefined).issuanceAsset();
        issuePset = takerSigner.sign(issuePset);
        issuePset = taker.finalize(issuePset);
        const issueTxid = await client.broadcastTx(issuePset.extractTx());
        await waitForTx(taker, client, issueTxid);
        assert.strictEqual(taker.balance().entries().get(asset.toString()), issuedAssetUnits);

        // ANCHOR: liquidex_make
        // (maker) Create a liquidex proposal (asking for the issued asset in exchange for the policy asset)
        const utxo = maker.utxos()[0].outpoint();
        let builder = new lwk.TxBuilder(network);
        builder = builder.liquidexMake(utxo, maker.address(null).address(), issuedAssetUnits, asset);
        const pset = builder.finish(maker);
        assert.strictEqual(pset.inputs()[0].sighash(), 131); // ANCHOR: ignore
        assert(maker.psetDetails(pset).hasNonDefaultSighash()); // ANCHOR: ignore
        const signedPset = makerSigner.sign(pset);

        // (maker) Create the proposal and convert it to string to pass it to the taker
        const proposal = lwk.UnvalidatedLiquidexProposal.fromPset(signedPset);
        const proposalStr = proposal.toString();
        // ANCHOR_END: liquidex_make

        // ANCHOR: liquidex_validate
        // (taker) Parse the proposal from string and validate it
        const proposalFromStr = lwk.UnvalidatedLiquidexProposal.new(proposalStr);
        const neededTxid = proposalFromStr.neededTx();
        const previousTx = await client.getTx(neededTxid);
        const validated = proposalFromStr.validate(previousTx);
        // ANCHOR_END: liquidex_validate

        // (taker) Verify proposal details
        assert.strictEqual(validated.input().amount(), utxoSize);
        assert.strictEqual(validated.output().amount(), issuedAssetUnits);
        assert.strictEqual(validated.input().asset().toString(), policyAsset.toString());
        assert.strictEqual(validated.output().asset().toString(), asset.toString());

        // ANCHOR: liquidex_take
        // (taker) Accept the proposal
        let builder2 = new lwk.TxBuilder(network).liquidexTake([validated]);
        const pset2 = builder2.finish(taker);
        assert.strictEqual(pset2.inputs()[1].sighash(), 1); // ANCHOR: ignore
        const signedPset2 = takerSigner.sign(pset2);

        // (taker) Finalize and broadcast the transaction
        const finalized = taker.finalize(signedPset2);
        const txid = await client.broadcastTx(finalized.extractTx());
        // ANCHOR_END: liquidex_take
        await waitForTx(maker, client, txid);
        await waitForTx(taker, client, txid);

        // Verify the final balance
        assert.strictEqual(maker.balance().entries().get(asset.toString()), issuedAssetUnits);
        assert.strictEqual(taker.balance().entries().get(asset.toString()) || 0n, 0n);
    } catch (error) {
        console.error("Liquidex test failed:", error);
        throw error;
    }
}

if (require.main === module) {
    runLiquidexTest().then(() => {
        console.log("liquidex.js: all tests passed");
    });
}

module.exports = { runLiquidexTest };
