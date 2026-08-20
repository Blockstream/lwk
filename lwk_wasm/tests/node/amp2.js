const assert = require('assert');
const lwk = require('lwk_node');
const { waitForTx, fundAddress, WATERFALLS_URL } = require('./scripts/utils.js');

const AMP2_URL = process.env.AMP2_URL || 'http://127.0.0.1:5000';

async function runAmp2Test() {
    try {
        const network = lwk.Network.regtestDefault();
        const client = new lwk.WaterfallsClient(network, WATERFALLS_URL);

        // from amp2 mock
        const serverKey = "[18778d8c]tpubD6NzVbkrYhZ4XpebgtJWPiggq824Dv4Y7zzdAqRn1BcmoWH5ff163zmB98FY1tEg518yPNG8hm3ghnHJwQ1qxynTDS7orfzQwwz45H9Hx7c";
        const amp2 = lwk.Amp2.new(serverKey, AMP2_URL);

        const mnemonic = new lwk.Mnemonic("abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about");
        const signer = new lwk.Signer(mnemonic, network);

        // e2e with elip153 desc
        const account = 0;
        const desc = amp2.elip153FromSigner(signer, account);

        // getting the descriptor with signer managed externally leads to the same descriptor
        const userPath = amp2.elip153UserPath(account);
        const viewPath = amp2.elip153ViewPath(account);
        const userKeyoriginXpub = signer.deriveXpub(userPath);
        const viewKeyoriginXpub = signer.deriveXpub(viewPath);
        const descFromStr = amp2.elip153FromExternalSigner(account, userKeyoriginXpub, viewKeyoriginXpub);
        assert.strictEqual(desc.descriptor().toString(), descFromStr.descriptor().toString());

        const wollet = new lwk.Wollet(network, desc.descriptor());

        const dwid = await amp2.register(desc);
        assert.strictEqual(dwid, wollet.dwid());

        const fundTxid = await fundAddress(wollet.address(0).address(), BigInt(100_000), network, client);
        await waitForTx(wollet, client, fundTxid);

        const sentSats = BigInt(1000);
        const recipient = wollet.address(null).address();

        let builder = new lwk.TxBuilder(network);
        builder = builder.addLbtcRecipient(recipient, sentSats);
        let pset = builder.finish(wollet);

        pset = signer.sign(pset);
        pset = await amp2.cosign(pset);

        pset = wollet.finalize(pset);

        const txid = await client.broadcastTx(pset.extractTx());
        await waitForTx(wollet, client, txid);
    } catch (error) {
        console.error("AMP2 test failed:", error);
        throw error;
    }
}

if (require.main === module) {
    runAmp2Test().then(() => {
        console.log("amp2.js: all tests passed");
    });
}

module.exports = { runAmp2Test };
