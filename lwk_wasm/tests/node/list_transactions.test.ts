import { expect } from "chai";
import * as lwk from "lwk_node";

describe("ListTransactions", function () {
  it("should list transactions and compare full scan vs UTXO-only scan", async function () {
    const mnemonic = new lwk.Mnemonic(
      "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about"
    );
    const network = lwk.Network.testnet();
    expect(network.toString()).to.equal("LiquidTestnet");

    const client = new lwk.EsploraClient(
      network,
      "https://waterfalls.liquidwebwallet.org/liquidtestnet/api",
      true,
      4,
      false
    );

    const signer = new lwk.Signer(mnemonic, network);
    const desc = signer.wpkhSlip77Descriptor();

    expect(desc.toString()).to.equal(
      "ct(slip77(9c8e4f05c7711a98c838be228bcb84924d4570ca53f35fa1c793e58841d47023),elwpkh([73c5da0a/84'/1'/0']tpubDC8msFGeGuwnKG9Upg7DM2b4DaRqg3CUZa5g8v2SRQ6K4NSkxUgd7HsL2XVWbVm39yBA4LAxysQAm397zwQSQoQgewGiYZqrA9DsP4zbQ1M/<0;1>/*))#2e4n992d"
    );

    const wollet = new lwk.Wollet(network, desc);
    const update = await client.fullScan(wollet);
    if (update) {
      wollet.applyUpdate(update);
    }

    const txs = wollet.transactions();
    expect(txs.length).to.be.at.least(99);
    const balance = wollet.balance();

    // Fetch transactions using waterfalls and utxos only
    const client_utxo_only = new lwk.EsploraClient(
      network,
      "https://waterfalls.liquidwebwallet.org/liquidtestnet/api",
      true,
      4,
      true
    );
    const wollet_utxo_only = new lwk.Wollet(network, desc);
    const update_utxo_only = await client_utxo_only.fullScan(wollet_utxo_only);
    if (update_utxo_only) {
      wollet_utxo_only.applyUpdate(update_utxo_only);
    }

    const txs_utxo_only = wollet_utxo_only.transactions();
    expect(txs_utxo_only.length).to.be.lessThan(txs.length);
    const balance_utxo_only = wollet_utxo_only.balance();

    const lbtc = network.policyAsset().toString();
    const lbtc_balance = balance[lbtc];
    const lbtc_utxo_only_balance = balance_utxo_only[lbtc];
    expect(lbtc_balance).to.equal(lbtc_utxo_only_balance);

    // Remove L-BTC from balances for comparison
    delete balance[lbtc];
    delete balance_utxo_only[lbtc];

    // Compare remaining balances
    expect(JSON.stringify(balance)).to.equal(JSON.stringify(balance_utxo_only));
  });
});
