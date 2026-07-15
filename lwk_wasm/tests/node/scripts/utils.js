const lwk = require("lwk_node");

const WATERFALLS_URL = process.env.WATERFALLS_URL || 'http://localhost:3000';

async function sync(wollet, client) {
  const update = await client.fullScan(wollet);
  if (update) {
    wollet.applyUpdate(update);
  }
}

async function waitForTx(wollet, client, txid) {
  const expectedTxid = typeof txid === "string" ? txid : txid.toString();
  for (let i = 0; i < 120; i++) {
    await sync(wollet, client);
    const list = wollet.txs(lwk.TxsOpt.default());
    if (list.some((e) => e.txid().toString() === expectedTxid)) {
      return;
    }
    await new Promise((resolve) => setTimeout(resolve, 500));
  }
  throw new Error(`Wallet does not have ${expectedTxid} in its list`);
}

async function fundAddress(address, amount, network, client) {
  const mnemonic = new lwk.Mnemonic("abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about");
  const signer = new lwk.Signer(mnemonic, network);
  const desc = signer.wpkhSlip77Descriptor();
  const wollet = new lwk.Wollet(network, desc);
  const update = await client.fullScan(wollet);
  if (update) wollet.applyUpdate(update);

  const builder = new lwk.TxBuilder(network)
    .addRecipient(address, amount, network.policyAsset());
  var pset = builder.finish(wollet);
  pset = signer.sign(pset);
  pset = wollet.finalize(pset);
  return await client.broadcastTx(pset.extractTx());
}

function generateAddress() {
  const network = lwk.Network.regtestDefault();
  const mnemonic = lwk.Mnemonic.fromRandom(12);
  const signer = new lwk.Signer(mnemonic, network);
  const desc = signer.wpkhSlip77Descriptor();
  const wollet = new lwk.Wollet(network, desc);
  return wollet.address(null).address()
}

module.exports = {
  sync,
  waitForTx,
  fundAddress,
  generateAddress,
  WATERFALLS_URL
};
