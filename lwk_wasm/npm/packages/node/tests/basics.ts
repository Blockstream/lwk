import * as assert from "node:assert/strict";
import * as lwk from "@blockstream/lwk-node";

// TODO: use regtest instead of testnet:
// however keep displaying testnet in the generated docs using // ANCHOR: ignore

export async function runBasicsTest(): Promise<void> {
  try {
    // ANCHOR: generate-signer
    if (false) {
      // ANCHOR: ignore
      const randomMnemonic = lwk.Mnemonic.fromRandom(12);
      void randomMnemonic;
    } // ANCHOR: ignore
    // Fixed mnemonic with some funds // ANCHOR: ignore
    const mnemonic = new lwk.Mnemonic(
      "other august catalog large suit off fan hammer ritual sword evil scrub"
    ); // ANCHOR: ignore
    const network = lwk.Network.testnet();
    const signer = new lwk.Signer(mnemonic, network);
    // ANCHOR_END: generate-signer

    // ANCHOR: get-xpub
    const xpub = signer.keyoriginXpub(lwk.Bip.bip84());
    // ANCHOR_END: get-xpub
    assert.ok(xpub);

    // ANCHOR: wollet
    const desc = signer.wpkhSlip77Descriptor();
    const wollet = new lwk.Wollet(network, desc);
    // ANCHOR_END: wollet

    // ANCHOR: address
    const addr = wollet.address(null).address().toString();
    // ANCHOR_END: address
    assert.ok(addr.length > 0);

    // ANCHOR: txs
    const txs = wollet.transactions();
    const balance = wollet.balance();
    // ANCHOR_END: txs
    assert.ok(Array.isArray(txs));
    assert.ok(balance);

    // TODO: move example code related to clients.md to a separate file `clients.ts`.
    // ANCHOR: esplora_client
    const urlEsplora = "https://blockstream.info/liquid/api";
    const esploraClient = new lwk.EsploraClient(
      lwk.Network.mainnet(),
      urlEsplora,
      true,
      4,
      false
    );
    // ANCHOR_END: esplora_client
    assert.ok(esploraClient);

    // ANCHOR: waterfalls_client
    const urlWaterfalls = "https://waterfalls.liquidwebwallet.org/liquid/api";
    const waterfallsClient = new lwk.EsploraClient(
      lwk.Network.mainnet(),
      urlWaterfalls,
      true,
      4,
      false
    );
    // ANCHOR_END: waterfalls_client
    assert.ok(waterfallsClient);

    // ANCHOR: client
    const url = "https://waterfalls.liquidwebwallet.org/liquidtestnet/api";
    // TODO: name variables // ANCHOR: ignore
    const client = new lwk.EsploraClient(network, url, true, 4, false);

    const update = await client.fullScan(wollet);
    if (update) {
      wollet.applyUpdate(update);
    }
    // ANCHOR_END: client

    // ANCHOR: tx
    const sats = 1000n;
    if (false) {
      // ANCHOR: ignore
      const manualAddress = new lwk.Address("<address>");
      const manualAsset = lwk.AssetId.fromString("<asset>");
      void manualAddress;
      void manualAsset;
    } // ANCHOR: ignore
    const address = wollet.address(null).address(); // ANCHOR: ignore
    const asset = network.policyAsset(); // ANCHOR: ignore

    let builder = new lwk.TxBuilder(network);
    builder = builder.addRecipient(address, sats, asset);
    let pset = builder.finish(wollet);
    // ANCHOR_END: tx

    // ANCHOR: pset-details
    const details = wollet.psetDetails(pset);
    // ANCHOR_END: pset-details
    assert.ok(details);

    // ANCHOR: sign
    pset = signer.sign(pset);
    // ANCHOR_END: sign

    // ANCHOR: broadcast
    pset = wollet.finalize(pset);
    const tx = pset.extractTx();
    const txid = await client.broadcastTx(tx);

    // (optional)
    wollet.applyTransaction(tx);
    // ANCHOR_END: broadcast

    console.log(txid.toString());
  } catch (error) {
    console.error("Basics test failed:", error);
    throw error;
  }
}
