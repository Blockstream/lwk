const lwk = require('lwk_node');
const { fundAddress, waitForTx, WATERFALLS_URL } = require('./scripts/utils');

async function runIssueAssetTest() {
  try {
    // ANCHOR: test_issue_asset
    const network = lwk.Network.regtestDefault();
    const policyAsset = network.policyAsset();
    const client = new lwk.WaterfallsClient(network, WATERFALLS_URL);

    // Create wallet
    const mnemonic = new lwk.Mnemonic("abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about");

    const signer = new lwk.Signer(mnemonic, network);
    const desc = signer.wpkhSlip77Descriptor();

    const wollet = new lwk.Wollet(network, desc);
    const wolletAddressResult = wollet.address(0);
    const wolletAddress = wolletAddressResult.address();

    const fundedSatoshi = 100000; // ANCHOR: ignore
    const fundTxid = await fundAddress(wolletAddress, BigInt(fundedSatoshi), network, client); // ANCHOR: ignore
    await waitForTx(wollet, client, fundTxid); // ANCHOR: ignore

    // ANCHOR: contract
    const contract = new lwk.Contract(
      "ciao.it",
      "0337cceec0beea0232ebe14cba0197a9fbd45fcf2ec946749de920e71434c2b904",
      "name",
      8,
      "TTT",
      0,
    );
    console.assert(contract.toString() === '{"entity":{"domain":"ciao.it"},"issuer_pubkey":"0337cceec0beea0232ebe14cba0197a9fbd45fcf2ec946749de920e71434c2b904","name":"name","precision":8,"ticker":"TTT","version":0}'); // ANCHOR: ignore
    // ANCHOR_END: contract

    // ANCHOR: issue_asset
    const issuedAsset = BigInt(10000);
    const reissuanceTokens = BigInt(1);

    // Create an issuance transaction
    var builder = new lwk.TxBuilder(network);
    builder = builder.issueAsset(issuedAsset, wolletAddress, reissuanceTokens, wolletAddress, contract);
    var unsignedPset = builder.finish(wollet);
    // ANCHOR_END: issue_asset
    // Sign the transaction and finalize it
    var signedPset = signer.sign(unsignedPset);
    var finalizedPset = wollet.finalize(signedPset);
    var tx = finalizedPset.extractTx();

    // Broadcast the transaction
    var txid = await client.broadcastTx(tx);

    // ANCHOR: issuance_ids
    var assetId = finalizedPset.inputs()[0].issuanceAsset();
    var tokenId = finalizedPset.inputs()[0].issuanceToken();
    // ANCHOR_END: issuance_ids
    // ANCHOR_END: test_issue_asset

    const issuance = finalizedPset.inputs()[0].issuance();
    console.assert(issuance.asset().toString() === assetId.toString());
    console.assert(issuance.token().toString() === tokenId.toString());
    console.assert(issuance.isIssuance());
    console.assert(!issuance.isReissuance());

    await waitForTx(wollet, client, txid);

    console.assert(wollet.balance().entries().get(assetId.toString()) === issuedAsset);
    console.assert(wollet.balance().entries().get(tokenId.toString()) === reissuanceTokens);

    // ANCHOR: reissue_asset
    const reissueAsset = BigInt(100);
    var assetReceiver = null; // Send the asset to the wollet creating the PSET
    var issuanceTx = null; // issuance transaction is present in the same wallet
    var builder = new lwk.TxBuilder(network);
    builder = builder.reissueAsset(assetId, reissueAsset, assetReceiver, issuanceTx);
    var unsignedPset = builder.finish(wollet);
    var signedPset = signer.sign(unsignedPset);
    var finalizedPset = wollet.finalize(signedPset);
    var tx = finalizedPset.extractTx();
    var txid = await client.broadcastTx(tx);
    // ANCHOR_END: reissue_asset

    const unsignedInputs = finalizedPset.inputs();
    const reissuanceIssuance = unsignedInputs.find(e => e.issuance());
    console.assert(reissuanceIssuance.issuance().asset().toString() === assetId.toString());
    console.assert(reissuanceIssuance.issuance().token().toString() === tokenId.toString());
    console.assert(!reissuanceIssuance.issuance().isIssuance());
    console.assert(reissuanceIssuance.issuance().isReissuance());

    await waitForTx(wollet, client, txid);

    const balanceAfterReissue = wollet.balance().entries().get(assetId.toString());
    console.assert(balanceAfterReissue === issuedAsset + reissueAsset);

    // ANCHOR: burn_asset
    const burnAsset = BigInt(50);
    var builder = new lwk.TxBuilder(network);
    builder = builder.addBurn(burnAsset, assetId);
    var unsignedPset = builder.finish(wollet);
    var signedPset = signer.sign(unsignedPset);
    var finalizedPset = wollet.finalize(signedPset);
    var tx = finalizedPset.extractTx();
    var txid = await client.broadcastTx(tx);
    // ANCHOR_END: burn_asset

    await waitForTx(wollet, client, txid);

    const balanceAfterBurn = wollet.balance().entries().get(assetId.toString());
    console.assert(balanceAfterBurn === issuedAsset + reissueAsset - burnAsset);

    console.log("Issue asset test passed");
  } catch (error) {
    console.error("Issue asset test failed:", error);
    throw error;
  }
}

if (require.main === module) {
  runIssueAssetTest();
}

module.exports = { runIssueAssetTest };
