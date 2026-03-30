import * as assert from "node:assert/strict";
import * as lwk from "@blockstream/lwk-node";

// TODO: use regtest instead of testnet:
// however keep displaying testnet in the generated docs using // ANCHOR: ignore

export async function runMultisigTest(): Promise<void> {
  try {
    // ANCHOR: multisig-setup
    const network = lwk.Network.testnet();
    // Derivation for multisig
    const bip = lwk.Bip.bip87();

    // Alice creates their signer and gets the xpub
    if (false) {
      // ANCHOR: ignore
      const randomMnemonicAlice = lwk.Mnemonic.fromRandom(12);
      void randomMnemonicAlice;
    } // ANCHOR: ignore
    // Fixed mnemonic with some funds // ANCHOR: ignore
    const mnemonicAlice = new lwk.Mnemonic(
      "kind they sing appear whip boil divorce essence mask alien teach wire"
    ); // ANCHOR: ignore
    const signerAlice = new lwk.Signer(mnemonicAlice, network);
    const xpubAlice = signerAlice.keyoriginXpub(bip);

    // Bob creates their signer and gets the xpub
    if (false) {
      // ANCHOR: ignore
      const randomMnemonicBob = lwk.Mnemonic.fromRandom(12);
      void randomMnemonicBob;
    } // ANCHOR: ignore
    // Fixed mnemonic with some funds // ANCHOR: ignore
    const mnemonicBob = new lwk.Mnemonic(
      "vast response truth other mansion skull hold amused capital satoshi oxygen brass"
    ); // ANCHOR: ignore
    const signerBob = new lwk.Signer(mnemonicBob, network);
    const xpubBob = signerBob.keyoriginXpub(bip);

    // Carol, who acts as a coordinator, creates their signer and gets the xpub
    if (false) {
      // ANCHOR: ignore
      const randomMnemonicCarol = lwk.Mnemonic.fromRandom(12);
      void randomMnemonicCarol;
    } // ANCHOR: ignore
    // Fixed mnemonic with some funds // ANCHOR: ignore
    const mnemonicCarol = new lwk.Mnemonic(
      "fresh inner begin grid symbol congress wall outer mass enable coil repeat"
    ); // ANCHOR: ignore
    const signerCarol = new lwk.Signer(mnemonicCarol, network);
    const xpubCarol = signerCarol.keyoriginXpub(bip);

    // Carol generates a random SLIP77 descriptor blinding key
    if (false) {
      // ANCHOR: ignore
      const randomSlip77Key = "<random-64-hex-chars>";
      void randomSlip77Key;
    } // ANCHOR: ignore
    const slip77RandKey =
      "1111111111111111111111111111111111111111111111111111111111111111"; // ANCHOR: ignore
    const descBlindingKey = `slip77(${slip77RandKey})`;

    // Carol uses the collected xpubs and the descriptor blinding key to create
    // the 2of3 descriptor
    const threshold = 2;
    const desc = `ct(${descBlindingKey},elwsh(multi(${threshold},${xpubAlice}/<0;1>/*,${xpubBob}/<0;1>/*,${xpubCarol}/<0;1>/*)))`;
    // Validate the descriptor string
    const wd = new lwk.WolletDescriptor(desc);
    // ANCHOR_END: multisig-setup

    // ANCHOR: multisig-receive
    // Carol creates the wollet
    const wolletCarol = new lwk.Wollet(network, wd);

    // With the wollet, Carol can obtain addresses, transactions and balance
    const addr = wolletCarol.address(null).address().toString();
    const txs = wolletCarol.transactions();
    const balance = wolletCarol.balance();
    void txs;
    void balance;

    // Update the wollet state
    const url = "https://waterfalls.liquidwebwallet.org/liquidtestnet/api";
    // TODO: name variables // ANCHOR: ignore
    const client = new lwk.EsploraClient(network, url, true, 4, false);

    const update = await client.fullScan(wolletCarol);
    if (update) {
      wolletCarol.applyUpdate(update);
    }
    // ANCHOR_END: multisig-receive

    const lbtc = network.policyAsset().toString();
    const lbtcBalance =
      new Map<string, bigint>(wolletCarol.balance().entries()).get(lbtc) ?? 0n;
    if (lbtcBalance < 500n) {
      console.log(
        `Balance is insufficient to make a transaction, send some tLBTC to ${addr}`
      );
      return;
    }

    // ANCHOR: multisig-send
    // Carol creates a transaction send few sats to a certain address
    const sats = 100n;
    if (false) {
      // ANCHOR: ignore
      const manualAddress = new lwk.Address("<address>");
      const manualAsset = lwk.AssetId.fromString("<asset>");
      void manualAddress;
      void manualAsset;
    } // ANCHOR: ignore
    const address = new lwk.Address(
      "tlq1qq2g07nju42l0nlx0erqa3wsel2l8prnq96rlnhml262mcj7pe8w6ndvvyg237japt83z24m8gu4v3yfhaqvrqxydadc9scsmw"
    ); // ANCHOR: ignore
    const asset = network.policyAsset(); // ANCHOR: ignore

    let builder = new lwk.TxBuilder(network);
    builder = builder.addRecipient(address, sats, asset);
    let pset = builder.finish(wolletCarol);

    pset = signerCarol.sign(pset);

    // Carol sends the PSET to Bob
    // Bob wants to analyze the PSET before signing, thus he creates a wollet
    const wdBob = new lwk.WolletDescriptor(desc);
    const wolletBob = new lwk.Wollet(network, wdBob);
    const updateBob = await client.fullScan(wolletBob);
    if (updateBob) {
      wolletBob.applyUpdate(updateBob);
    }
    // Then Bob uses the wollet to analyze the PSET
    const details = wolletBob.psetDetails(pset);
    // PSET has a reasonable fee
    assert.ok(details.balance().fee() < 100);
    // PSET has a signature from Carol
    assert.equal(details.fingerprintsHas().length, 1);
    assert.ok(details.fingerprintsHas().includes(signerCarol.fingerprint()));
    // PSET needs a signature from either Bob or Carol
    assert.equal(details.fingerprintsMissing().length, 2);
    assert.ok(
      details.fingerprintsMissing().includes(signerAlice.fingerprint())
    );
    assert.ok(details.fingerprintsMissing().includes(signerBob.fingerprint()));
    // PSET has a single recipient, with data matching what was specified above
    assert.equal(details.balance().recipients().length, 1);
    const recipient = details.balance().recipients()[0];
    if (!recipient) {
      throw new Error("Expected a single recipient in PSET details");
    }
    const recipientAddress = recipient.address();
    const recipientAsset = recipient.asset();
    const recipientValue = recipient.value();
    if (!recipientAddress || !recipientAsset || recipientValue === undefined) {
      throw new Error("Recipient details are incomplete");
    }
    assert.equal(recipientAddress.toString(), address.toString());
    assert.equal(recipientAsset.toString(), asset.toString());
    assert.equal(recipientValue, sats);

    // Bob is satisfied with the PSET and signs it
    pset = signerBob.sign(pset);

    // Bob sends the PSET back to Carol
    // Carol checks that the PSET has enough signatures
    const detailsBob = wolletBob.psetDetails(pset);
    assert.equal(detailsBob.fingerprintsHas().length, 2);

    // Carol finalizes the PSET and broadcast the transaction
    pset = wolletCarol.finalize(pset);
    const tx = pset.extractTx();
    const txid = await client.broadcastTx(tx);
    // ANCHOR_END: multisig-send
    console.log(txid.toString());
  } catch (error) {
    console.error("Multisig test failed:", error);
    throw error;
  }
}
