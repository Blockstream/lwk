import * as assert from "node:assert/strict";
import * as lwk from "@blockstream/lwk-node";

export async function runListTransactionsTest(): Promise<void> {
  try {
    console.log("Starting list transactions test");

    const mnemonic = new lwk.Mnemonic(
      "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about"
    );
    const network = lwk.Network.testnet();
    assert.equal(network.toString(), "LiquidTestnet");

    const client = new lwk.EsploraClient(
      network,
      "https://waterfalls.liquidwebwallet.org/liquidtestnet/api",
      true,
      4,
      false
    );

    const signer = new lwk.Signer(mnemonic, network);
    const desc = signer.wpkhSlip77Descriptor();

    assert.equal(
      desc.toString(),
      "ct(slip77(9c8e4f05c7711a98c838be228bcb84924d4570ca53f35fa1c793e58841d47023),elwpkh([73c5da0a/84'/1'/0']tpubDC8msFGeGuwnKG9Upg7DM2b4DaRqg3CUZa5g8v2SRQ6K4NSkxUgd7HsL2XVWbVm39yBA4LAxysQAm397zwQSQoQgewGiYZqrA9DsP4zbQ1M/<0;1>/*))#2e4n992d"
    );

    const wollet = new lwk.Wollet(network, desc);
    console.log("Starting full scan...");
    const update = await client.fullScan(wollet);
    if (update) {
      wollet.applyUpdate(update);
    }

    const txs = wollet.transactions();
    assert.ok(txs.length >= 99);
    const balanceEntries = new Map<string, bigint>(wollet.balance().entries());

    const clientUtxoOnly = new lwk.EsploraClient(
      network,
      "https://waterfalls.liquidwebwallet.org/liquidtestnet/api",
      true,
      4,
      true
    );
    const wolletUtxoOnly = lwk.Wollet.newUtxoOnly(network, desc);
    console.log("Starting UTXO-only full scan...");
    const updateUtxoOnly = await clientUtxoOnly.fullScan(wolletUtxoOnly);
    if (updateUtxoOnly) {
      wolletUtxoOnly.applyUpdate(updateUtxoOnly);
    }

    const txsUtxoOnly = wolletUtxoOnly.transactions();
    assert.ok(txsUtxoOnly.length < txs.length);
    const balanceUtxoOnlyEntries = new Map<string, bigint>(
      wolletUtxoOnly.balance().entries()
    );

    const lbtc = network.policyAsset().toString();
    const lbtcBalance = balanceEntries.get(lbtc);
    const lbtcUtxoOnlyBalance = balanceUtxoOnlyEntries.get(lbtc);

    assert.equal(lbtcBalance, lbtcUtxoOnlyBalance);

    balanceEntries.delete(lbtc);
    balanceUtxoOnlyEntries.delete(lbtc);

    assert.deepEqual(
      Array.from(balanceEntries.entries()),
      Array.from(balanceUtxoOnlyEntries.entries())
    );

    console.log("List transactions test passed!");
  } catch (error) {
    console.error("List transactions test failed:", error);
    throw error;
  }
}
