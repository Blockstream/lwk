const lwk = require('lwk_node');

async function runListTransactionsTest() {
    try {
        console.log("Starting list transactions test");

        const mnemonic = new lwk.Mnemonic("abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about");
        const network = lwk.Network.testnet();
        console.assert(network.toString() === "LiquidTestnet");

        const client = new lwk.EsploraClient(network, "https://waterfalls.liquidwebwallet.org/liquidtestnet/api", true, 4, false);

        const signer = new lwk.Signer(mnemonic, network);
        const desc = signer.wpkhSlip77Descriptor();

        console.assert(desc.toString() === "ct(slip77(9c8e4f05c7711a98c838be228bcb84924d4570ca53f35fa1c793e58841d47023),elwpkh([73c5da0a/84'/1'/0']tpubDC8msFGeGuwnKG9Upg7DM2b4DaRqg3CUZa5g8v2SRQ6K4NSkxUgd7HsL2XVWbVm39yBA4LAxysQAm397zwQSQoQgewGiYZqrA9DsP4zbQ1M/<0;1>/*))#2e4n992d");

        const wollet = new lwk.Wollet(network, desc);
        console.log("Starting full scan...");
        const update = await client.fullScan(wollet);
        if (update) {
            wollet.applyUpdate(update);
        }

        const txs = wollet.transactions();
        console.assert(txs.length >= 99);
        const opt = lwk.TxsOpt.default();
        const txs2 = wollet.txs(opt);
        console.assert(txs.length === txs2.length);
        const num_txs = wollet.numTxs();
        console.assert(txs.length === num_txs);

        const limit = 10;
        const page1 = wollet.transactionsPaginated(0, limit);
        console.assert(page1.length === limit);
        console.assert(JSON.stringify(page1.map(t => t.txid)) === JSON.stringify(txs.slice(0, limit).map(t => t.txid)));

        const offset = 15;
        const page2 = wollet.transactionsPaginated(offset, limit);
        console.assert(page2.length === limit);
        console.assert(JSON.stringify(page2.map(t => t.txid)) === JSON.stringify(txs.slice(offset, offset + limit).map(t => t.txid)));

        const page1_opt = lwk.TxsOpt.withPagination(0, limit);
        const page1_txs = wollet.txs(page1_opt);
        console.assert(page1_txs.length === limit);
        console.assert(JSON.stringify(page1_txs.map(t => t.txid)) === JSON.stringify(txs2.slice(0, limit).map(t => t.txid)));

        const page2_opt = lwk.TxsOpt.withPagination(offset, limit);
        const page2_txs = wollet.txs(page2_opt);
        console.assert(page2.length === limit);
        console.assert(JSON.stringify(page2_txs.map(t => t.txid)) === JSON.stringify(txs2.slice(offset, offset + limit).map(t => t.txid)));

        const balance = wollet.balance();

        const url = "https://blockstream.info/liquidtestnet/";
        const tx = txs[0];
        const txid = tx.txid();
        const txopt = lwk.TxOpt.default();
        const tx2 = wollet.txDetails(txid, txopt);
        const old_url = tx.unblindedUrl(url);
        const new_url = tx2.unblindedUrl(url);
        console.assert(old_url === new_url);

        // Fetch transactions using waterfalls and utxos only
        const client_utxo_only = new lwk.EsploraClient(network, "https://waterfalls.liquidwebwallet.org/liquidtestnet/api", true, 4, true);
        const wollet_utxo_only = new lwk.WolletBuilder(network, desc).utxoOnly(true).build();
        console.log("Starting UTXO-only full scan...");
        const update_utxo_only = await client_utxo_only.fullScan(wollet_utxo_only);
        if (update_utxo_only) {
            wollet_utxo_only.applyUpdate(update_utxo_only);
        }

        const txs_utxo_only = wollet_utxo_only.transactions();
        console.assert(txs_utxo_only.length < txs.length);
        const balance_utxo_only = wollet_utxo_only.balance();

        const lbtc = network.policyAsset().toString();
        const lbtc_balance = balance[lbtc];
        const lbtc_utxo_only_balance = balance_utxo_only[lbtc];
        console.assert(lbtc_balance === lbtc_utxo_only_balance);

        // Remove L-BTC from balances for comparison
        delete balance[lbtc];
        delete balance_utxo_only[lbtc];

        // Compare remaining balances
        console.assert(JSON.stringify(balance) === JSON.stringify(balance_utxo_only));

        console.log("List transactions test passed!");
    } catch (error) {
        console.error("List transactions test failed:", error);
        throw error;
    }
}

if (require.main === module) {
    runListTransactionsTest();
}

module.exports = { runListTransactionsTest };
