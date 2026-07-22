const assert = require('assert');
const fs = require('fs');
const lwk = require('lwk_node');

// TODO: migrate to the regtest setup 

function runManualCoinSelectionTest() {
    try {
        const descriptorString = fs
            .readFileSync(`${__dirname}/../../test_data/update_with_mnemonic/descriptor.txt`, 'utf8')
            .trim();
        const encryptedUpdate = fs
            .readFileSync(
                `${__dirname}/../../test_data/update_with_mnemonic/update_serialized_encrypted.txt`,
                'utf8'
            )
            .trim();

        const network = lwk.Network.testnet();
        const descriptor = new lwk.WolletDescriptor(descriptorString);
        const update = lwk.Update.deserializeDecryptedBase64(encryptedUpdate, descriptor);

        const wollet = new lwk.WolletBuilder(network, descriptor).build();
        wollet.applyUpdate(update);

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

function runInputOrderTest() {
    try {
        const descriptorString = fs
            .readFileSync(`${__dirname}/../../test_data/update_with_mnemonic/descriptor2.txt`, 'utf8')
            .trim();
        const encryptedUpdate = fs
            .readFileSync(
                `${__dirname}/../../test_data/update_with_mnemonic/update_serialized_encrypted2.txt`,
                'utf8'
            )
            .trim();

        const network = lwk.Network.testnet();
        const descriptor = new lwk.WolletDescriptor(descriptorString);
        const update = lwk.Update.deserializeDecryptedBase64(encryptedUpdate, descriptor);

        const wollet = new lwk.WolletBuilder(network, descriptor).build();
        wollet.applyUpdate(update);

        const utxos = wollet.utxos();
        const policyAsset = network.policyAsset();
        const lbtcUtxo = utxos.find((u) => u.unblinded().asset().toString() === policyAsset.toString());
        const assetUtxo = utxos.find((u) => u.unblinded().asset().toString() !== policyAsset.toString());
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

function runIssueAssetTest() {
    try {
        const descriptorString = fs
            .readFileSync(`${__dirname}/../../test_data/update_with_mnemonic/descriptor2.txt`, 'utf8')
            .trim();
        const encryptedUpdate = fs
            .readFileSync(
                `${__dirname}/../../test_data/update_with_mnemonic/update_serialized_encrypted2.txt`,
                'utf8'
            )
            .trim();

        const network = lwk.Network.testnet();
        const descriptor = new lwk.WolletDescriptor(descriptorString);
        const update = lwk.Update.deserializeDecryptedBase64(encryptedUpdate, descriptor);

        const wollet = new lwk.WolletBuilder(network, descriptor).build();
        wollet.applyUpdate(update);

        const utxos = wollet.utxos();
        assert.strictEqual(utxos.length, 2);
        const policyAsset = network.policyAsset();
        const lbtcUtxo = utxos.find((u) => u.unblinded().asset().toString() === policyAsset.toString());
        const assetUtxo = utxos.find((u) => u.unblinded().asset().toString() !== policyAsset.toString());

        // Two issuances in the same transaction, sequentially assigned to the wallet's two UTXOs
        const request1 = new lwk.IssuanceRequest(BigInt(1000), BigInt(1));
        const request2 = new lwk.IssuanceRequest(BigInt(2000), BigInt(2));
        let builder = new lwk.TxBuilder(network);
        builder = builder.addIssuance(request1);
        builder = builder.addIssuance(request2);
        builder = builder.setWalletUtxos(utxos.map((u) => u.outpoint()));
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

        // Two issuances in the same transaction, each pinned to a different one of the wallet's two UTXOs
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

function runTxBuilderTest() {
    runManualCoinSelectionTest();
    runInputOrderTest();
    runIssueAssetTest();
}

if (require.main === module) {
    runTxBuilderTest();
    console.log("tx_builder.js: all tests passed");
}

module.exports = { runTxBuilderTest };
