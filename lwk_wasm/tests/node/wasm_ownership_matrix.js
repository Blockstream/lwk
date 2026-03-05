'use strict';

const fs = require('fs');
const path = require('path');
const lwk = require('lwk_node');

const P2PK_SOURCE = fs.readFileSync(
  path.join(__dirname, '../../../lwk_simplicity/data/p2pk.simf'),
  'utf8'
);
const PSET_BASE64 = fs
  .readFileSync(
    path.join(__dirname, '../../../lwk_jade/test_data/pset_to_be_signed.base64'),
    'utf8'
  )
  .trim();

const TEST_PUBLIC_KEY =
  '8a65c55726dc32b59b649ad0187eb44490de681bb02601b8d3f58c8b9fff9083';
const TEST_SIGNATURE =
  'ab5173c154e62da5e4f5d983177d7398918d61d6149f3b1bb7271d00d165391c19f86c53c5d6dcfcd50f4fc4dbf8748fbb6a1614ddacd663818bfc90ef8f818d';
const TEST_UNSIGNED_TX =
  '02000000000113226c2af4a18516258790b9c6f118afdf0bfe9cb0cf79574807ddf6bf680be80000000000ffffffff0301499a818545f6bae39fc03b637f2a4e1e64e590cac1bc3a6f6d71aa4443654c140100000000000003e800225120f8b916e58321fccfb61529245bcae76bdf0cedbcb0df2b23c2ac8f1adfed577501499a818545f6bae39fc03b637f2a4e1e64e590cac1bc3a6f6d71aa4443654c140100000000000181be00225120f8b916e58321fccfb61529245bcae76bdf0cedbcb0df2b23c2ac8f1adfed577501499a818545f6bae39fc03b637f2a4e1e64e590cac1bc3a6f6d71aa4443654c140100000000000000fa000000000000';
const TEST_UTXO_SCRIPT_PUBKEY =
  '5120f8b916e58321fccfb61529245bcae76bdf0cedbcb0df2b23c2ac8f1adfed5775';
const TEST_ADDRESS =
  'tex1plzu3devry87vlds49yj9hjh8d00semdukr0jkg7z4j834hld2a6s6y4amk';
const TEST_OUTPOINT =
  '[elements]b93dbfb3fa1929b6f82ed46c4a5d8e1c96239ca8b3d9fce00c321d7dadbdf6e0:0';
const TEST_WANTED_ASSET =
  '38fca2d939696061a8f76d4e6b5eecd54e3b4221c846f24a6b279e79952850a5';
const TEST_CONTRACT_ISSUER_PUBKEY =
  '0337cceec0beea0232ebe14cba0197a9fbd45fcf2ec946749de920e71434c2b904';

function assert(condition, message) {
  if (!condition) {
    throw new Error(message);
  }
}

function expectMoved(label, fn) {
  let error = null;
  try {
    fn();
  } catch (e) {
    error = e;
  }
  if (!error) {
    throw new Error(`${label}: expected moved-object failure`);
  }
  const msg = String(error);
  if (!/null pointer|moved|use after|wrong type|JsValue\(null\)/i.test(msg)) {
    throw new Error(`${label}: unexpected error message: ${msg}`);
  }

  console.log(label, msg)
}

function loadUpdateFixture(fileName) {
  return fs
    .readFileSync(
      path.join(__dirname, `../../test_data/update_with_mnemonic/${fileName}`),
      'utf8'
    )
    .trim();
}

function buildFixtureContext() {
  const network = lwk.Network.testnet();
  const mnemonic = new lwk.Mnemonic(loadUpdateFixture('mnemonic.txt'));
  const descriptor = new lwk.WolletDescriptor(loadUpdateFixture('descriptor.txt'));
  const updateBase64 = loadUpdateFixture('update_serialized_encrypted.txt');

  const signer = new lwk.Signer(mnemonic, network);
  const wollet = new lwk.Wollet(network, descriptor);
  const update = lwk.Update.deserializeDecryptedBase64(updateBase64, descriptor);
  wollet.applyUpdate(update);

  return { network, signer, wollet };
}

function buildP2pkProgram() {
  let args = new lwk.SimplicityArguments();
  args = args.addValue(
    'PUBLIC_KEY',
    lwk.SimplicityTypedValue.fromU256Hex(TEST_PUBLIC_KEY)
  );
  return new lwk.SimplicityProgram(P2PK_SOURCE, args);
}

function makeSingleUtxoArray(network) {
  const script = new lwk.Script(TEST_UTXO_SCRIPT_PUBKEY);
  const utxo = lwk.TxOut.fromExplicit(script, network.policyAsset(), BigInt(100000));
  return { utxo, utxos: [utxo] };
}

function testSimplicityConsumption() {
  const { network, signer } = buildFixtureContext();
  const tx = lwk.Transaction.fromString(TEST_UNSIGNED_TX);
  const program = buildP2pkProgram();

  // SimplicityType::from_elements(elements: Vec<SimplicityType>)
  const t1 = lwk.SimplicityType.u32();
  const t2 = lwk.SimplicityType.u64();
  lwk.SimplicityType.fromElements([t1, t2]);
  expectMoved('SimplicityType::from_elements consumes elements', () => {
    lwk.SimplicityType.option(t1);
  });

  // SimplicityTypedValue::from_elements(elements: Vec<SimplicityTypedValue>)
  const v1 = lwk.SimplicityTypedValue.fromU32(1);
  const v2 = lwk.SimplicityTypedValue.fromU64(BigInt(2));
  lwk.SimplicityTypedValue.fromElements([v1, v2]);
  expectMoved('SimplicityTypedValue::from_elements consumes elements', () => {
    lwk.SimplicityTypedValue.some(v1);
  });

  // SimplicityArguments::add_value(mut self, name: &str, value: &SimplicityTypedValue) -> SimplicityArguments
  const args = new lwk.SimplicityArguments();
  const args2 = args.addValue('A', lwk.SimplicityTypedValue.fromU8(1));
  expectMoved('SimplicityArguments::add_value consumes self', () => {
    args.addValue('B', lwk.SimplicityTypedValue.fromU8(2));
  });
  args2.addValue('C', lwk.SimplicityTypedValue.fromU8(3));

  // SimplicityWitnessValues::add_value(mut self, name: &str, value: &SimplicityTypedValue) -> SimplicityWitnessValues
  const witness0 = new lwk.SimplicityWitnessValues();
  const witness1 = witness0.addValue('A', lwk.SimplicityTypedValue.fromBoolean(true));
  expectMoved('SimplicityWitnessValues::add_value consumes self', () => {
    witness0.addValue('B', lwk.SimplicityTypedValue.fromBoolean(false));
  });
  witness1.addValue(
    'SIGNATURE',
    lwk.SimplicityTypedValue.fromByteArrayHex(TEST_SIGNATURE)
  );

  // SimplicityProgram::get_sighash_all(..., utxos: Vec<TxOut>, ...)
  const sighashCase = makeSingleUtxoArray(network);
  program.getSighashAll(
    tx,
    new lwk.XOnlyPublicKey(TEST_PUBLIC_KEY),
    sighashCase.utxos,
    0,
    network
  );
  expectMoved('SimplicityProgram::get_sighash_all consumes TxOut elements', () => {
    sighashCase.utxo.value();
  });

  // SimplicityProgram::finalize_transaction(..., utxos: Vec<TxOut>, ...)
  const finalizeCase = makeSingleUtxoArray(network);
  let witnessFinalize = new lwk.SimplicityWitnessValues();
  witnessFinalize = witnessFinalize.addValue(
    'SIGNATURE',
    lwk.SimplicityTypedValue.fromByteArrayHex(TEST_SIGNATURE)
  );
  try {
    program.finalizeTransaction(
      tx,
      new lwk.XOnlyPublicKey(TEST_PUBLIC_KEY),
      finalizeCase.utxos,
      0,
      witnessFinalize,
      network,
      lwk.SimplicityLogLevel.None
    );
  } catch (_) {
    // Ownership behavior is independent from success/failure of execution.
  }
  expectMoved('SimplicityProgram::finalize_transaction consumes TxOut elements', () => {
    finalizeCase.utxo.value();
  });

  // SimplicityProgram::create_p2pk_signature(..., utxos: Vec<TxOut>, ...)
  const signCase = makeSingleUtxoArray(network);
  try {
    program.createP2pkSignature(
      signer,
      'm/84h/1h/0h/0/0',
      tx,
      signCase.utxos,
      0,
      network
    );
  } catch (_) {
    // Ownership behavior is independent from success/failure of execution.
  }
  expectMoved('SimplicityProgram::create_p2pk_signature consumes TxOut elements', () => {
    signCase.utxo.value();
  });

  // SimplicityProgram::run(..., utxos: Vec<TxOut>, ...)
  const runCase = makeSingleUtxoArray(network);
  let witnessRun = new lwk.SimplicityWitnessValues();
  witnessRun = witnessRun.addValue(
    'SIGNATURE',
    lwk.SimplicityTypedValue.fromByteArrayHex(TEST_SIGNATURE)
  );
  try {
    program.run(
      tx,
      new lwk.XOnlyPublicKey(TEST_PUBLIC_KEY),
      runCase.utxos,
      0,
      witnessRun,
      network,
      lwk.SimplicityLogLevel.None
    );
  } catch (_) {
    // Ownership behavior is independent from success/failure of execution.
  }
  expectMoved('SimplicityProgram::run consumes TxOut elements', () => {
    runCase.utxo.value();
  });
}

function testTxBuilderConsumption() {
  const network = lwk.Network.testnet();

  // TxBuilder::drain_lbtc_to(self, address: Address)
  let builderDrain = new lwk.TxBuilder(network);
  const drainAddress = new lwk.Address(TEST_ADDRESS);
  builderDrain = builderDrain.drainLbtcTo(drainAddress);
  expectMoved('TxBuilder::drain_lbtc_to consumes address', () => {
    drainAddress.toString();
  });
  assert(builderDrain.toString().length > 0, 'drain builder should still be valid');

  // TxBuilder::add_explicit_recipient(self, address: Address, ...)
  let builderExplicit = new lwk.TxBuilder(network);
  const explicitAddress = new lwk.Address(TEST_ADDRESS);
  try {
    builderExplicit = builderExplicit.addExplicitRecipient(
      explicitAddress,
      BigInt(1000),
      network.policyAsset()
    );
  } catch (_) {
    // Ownership behavior is independent from success/failure of execution.
  }
  expectMoved('TxBuilder::add_explicit_recipient consumes address', () => {
    explicitAddress.toString();
  });

  // TxBuilder::issue_asset(self, ..., Option<Address>, Option<Contract>)
  let builderIssue = new lwk.TxBuilder(network);
  const issueAssetReceiver = new lwk.Address(TEST_ADDRESS);
  const issueTokenReceiver = new lwk.Address(TEST_ADDRESS);
  const issueContract = new lwk.Contract(
    'ciao.it',
    TEST_CONTRACT_ISSUER_PUBKEY,
    'NAME',
    0,
    'NME',
    0
  );
  try {
    builderIssue = builderIssue.issueAsset(
      BigInt(1),
      issueAssetReceiver,
      BigInt(1),
      issueTokenReceiver,
      issueContract
    );
  } catch (_) {
    // Ownership behavior is independent from success/failure of execution.
  }
  expectMoved('TxBuilder::issue_asset consumes asset_receiver', () => {
    issueAssetReceiver.toString();
  });
  expectMoved('TxBuilder::issue_asset consumes token_receiver', () => {
    issueTokenReceiver.toString();
  });
  expectMoved('TxBuilder::issue_asset consumes contract', () => {
    issueContract.toString();
  });

  // TxBuilder::reissue_asset(self, asset_to_reissue: AssetId, ..., issuance_tx: Option<Transaction>)
  let builderReissue = new lwk.TxBuilder(network);
  const reissueAsset = network.policyAsset();
  const reissueReceiver = new lwk.Address(TEST_ADDRESS);
  const issuanceTx = lwk.Transaction.fromString(TEST_UNSIGNED_TX);
  try {
    builderReissue = builderReissue.reissueAsset(
      reissueAsset,
      BigInt(1),
      reissueReceiver,
      issuanceTx
    );
  } catch (_) {
    // Ownership behavior is independent from success/failure of execution.
  }
  expectMoved('TxBuilder::reissue_asset consumes asset_to_reissue', () => {
    reissueAsset.toString();
  });
  expectMoved('TxBuilder::reissue_asset consumes asset_receiver', () => {
    reissueReceiver.toString();
  });
  expectMoved('TxBuilder::reissue_asset consumes issuance_tx', () => {
    issuanceTx.toString();
  });

  // TxBuilder::set_wallet_utxos(self, outpoints: Vec<OutPoint>)
  let builderSetUtxos = new lwk.TxBuilder(network);
  const outpoint = new lwk.OutPoint(TEST_OUTPOINT);
  builderSetUtxos = builderSetUtxos.setWalletUtxos([outpoint]);
  expectMoved('TxBuilder::set_wallet_utxos consumes OutPoint elements', () => {
    outpoint.vout();
  });
  assert(builderSetUtxos.toString().length > 0, 'utxo builder should still be valid');

  // TxBuilder::liquidex_make(self, utxo: OutPoint, address: Address, asset_id: AssetId)
  let builderLiquidexMake = new lwk.TxBuilder(network);
  const makeOutpoint = new lwk.OutPoint(TEST_OUTPOINT);
  const makeAddress = new lwk.Address(TEST_ADDRESS);
  const makeAsset = network.policyAsset();
  try {
    builderLiquidexMake = builderLiquidexMake.liquidexMake(
      makeOutpoint,
      makeAddress,
      BigInt(1),
      makeAsset
    );
  } catch (_) {
    // Ownership behavior is independent from success/failure of execution.
  }
  expectMoved('TxBuilder::liquidex_make consumes utxo', () => {
    makeOutpoint.vout();
  });
  expectMoved('TxBuilder::liquidex_make consumes address', () => {
    makeAddress.toString();
  });
  expectMoved('TxBuilder::liquidex_make consumes asset_id', () => {
    makeAsset.toString();
  });
}

function testPsetScriptRegistryPostConsumption() {
  const network = lwk.Network.testnet();

  // Pset::combine(&mut self, other: Pset)
  const pset1 = new lwk.Pset(PSET_BASE64);
  const pset2 = new lwk.Pset(PSET_BASE64);
  pset1.combine(pset2);
  expectMoved('Pset::combine consumes other', () => {
    pset2.toString();
  });

  // PsetBuilder::blind_last(..., secrets: Vec<TxOutSecrets>)
  const secret = lwk.TxOutSecrets.fromExplicit(network.policyAsset(), BigInt(1000));
  let gotBlindLastError = false;
  try {
    lwk.PsetBuilder.newV2().blindLast(new Uint32Array([]), [secret]);
  } catch (_) {
    gotBlindLastError = true;
  }
  assert(gotBlindLastError, 'blindLast should fail for mismatched inputs');
  expectMoved('PsetBuilder::blind_last consumes TxOutSecrets elements', () => {
    secret.value();
  });

  // RegistryPost::new(contract: Contract, asset_id: AssetId)
  const contract = new lwk.Contract(
    'ciao.it',
    TEST_CONTRACT_ISSUER_PUBKEY,
    'NAME',
    0,
    'NME',
    0
  );
  const registryAsset = network.policyAsset();
  const post = new lwk.RegistryPost(contract, registryAsset);
  assert(post.toString().length > 0, 'RegistryPost should be created');
  expectMoved('RegistryPost::new consumes contract', () => {
    contract.toString();
  });
  expectMoved('RegistryPost::new consumes asset_id', () => {
    registryAsset.toString();
  });

  // Script::is_provably_segwit(&self, redeem_script: Option<Script>)
  const scriptPubkey = new lwk.Script(TEST_UTXO_SCRIPT_PUBKEY);
  const redeemScript = new lwk.Script('51');
  scriptPubkey.isProvablySegwit(redeemScript);
  expectMoved('Script::is_provably_segwit consumes redeem_script', () => {
    redeemScript.toString();
  });
}

function testPsetConsumersUsingFixture() {
  const { network, signer, wollet } = buildFixtureContext();

  // Signer::sign(&self, pset: Pset)
  const signerPset = new lwk.Pset(PSET_BASE64);
  try {
    signer.sign(signerPset);
  } catch (_) {
    // Ownership behavior is independent from success/failure of execution.
  }
  expectMoved('Signer::sign consumes pset', () => {
    signerPset.toString();
  });

  // SecretKey::sign(&self, pset: Pset)
  const secretKey = new lwk.SecretKey(new Uint8Array(32).fill(0xcd));
  const secretKeyPset = new lwk.Pset(PSET_BASE64);
  try {
    secretKey.sign(secretKeyPset);
  } catch (_) {
    // Ownership behavior is independent from success/failure of execution.
  }
  expectMoved('SecretKey::sign consumes pset', () => {
    secretKeyPset.toString();
  });

  // Wollet::finalize(&self, pset: Pset)
  const finalizePset = new lwk.Pset(PSET_BASE64);
  try {
    wollet.finalize(finalizePset);
  } catch (_) {
    // Ownership behavior is independent from success/failure of execution.
  }
  expectMoved('Wollet::finalize consumes pset', () => {
    finalizePset.toString();
  });

  // Registry::add_contracts(&self, pset: Pset)
  const registry = lwk.Registry.defaultHardcodedForNetwork(network);
  const addContractsPset = new lwk.Pset(PSET_BASE64);
  try {
    registry.addContracts(addContractsPset);
  } catch (_) {
    // Ownership behavior is independent from success/failure of execution.
  }
  expectMoved('Registry::add_contracts consumes pset', () => {
    addContractsPset.toString();
  });
}

function testLiquidexProposalConsumption() {
  const { network, signer, wollet } = buildFixtureContext();

  const utxos = wollet.utxos();
  assert(utxos.length > 0, 'fixture wallet must contain utxos');

  const utxo = utxos[0].outpoint();
  const addr = wollet.address(null).address();
  const wantedAsset = lwk.AssetId.fromString(TEST_WANTED_ASSET);

  let psetMaker = new lwk.TxBuilder(network)
    .liquidexMake(utxo, addr, BigInt(1), wantedAsset)
    .finish(wollet);
  psetMaker = signer.sign(psetMaker);
  const signedMakerBase64 = psetMaker.toString();

  // UnvalidatedLiquidexProposal::from_pset(pset: Pset)
  const psetForFromPset = new lwk.Pset(signedMakerBase64);
  lwk.UnvalidatedLiquidexProposal.fromPset(psetForFromPset);
  expectMoved('UnvalidatedLiquidexProposal::from_pset consumes pset', () => {
    psetForFromPset.toString();
  });

  // UnvalidatedLiquidexProposal::validate(self, tx: Transaction)
  const psetForValidate = new lwk.Pset(signedMakerBase64);
  const unvalidatedForValidate =
    lwk.UnvalidatedLiquidexProposal.fromPset(psetForValidate);
  const validateTx = lwk.Transaction.fromString(TEST_UNSIGNED_TX);
  try {
    unvalidatedForValidate.validate(validateTx);
  } catch (_) {
    // Ownership behavior is independent from success/failure of execution.
  }
  expectMoved('UnvalidatedLiquidexProposal::validate consumes self', () => {
    unvalidatedForValidate.toString();
  });
  expectMoved('UnvalidatedLiquidexProposal::validate consumes tx', () => {
    validateTx.toString();
  });

  // TxBuilder::liquidex_take(self, proposals: Vec<ValidatedLiquidexProposal>)
  const psetForTake = new lwk.Pset(signedMakerBase64);
  const unvalidatedForTake = lwk.UnvalidatedLiquidexProposal.fromPset(psetForTake);
  const validated = unvalidatedForTake.insecureValidate();
  new lwk.TxBuilder(network).liquidexTake([validated]);
  expectMoved('TxBuilder::liquidex_take consumes proposal elements', () => {
    validated.toString();
  });
}

function testAmp0PsetConsumptionIfAvailable() {
  if (typeof lwk.Amp0Pset !== 'function') {
    return;
  }

  // Amp0Pset::new(pset: Pset, ...)
  const pset = new lwk.Pset(PSET_BASE64);
  new lwk.Amp0Pset(pset, []);
  expectMoved('Amp0Pset::new consumes pset', () => {
    pset.toString();
  });
}

function runOwnershipMatrix() {
  testSimplicityConsumption();
  testTxBuilderConsumption();
  testPsetScriptRegistryPostConsumption();
  testPsetConsumersUsingFixture();
  testLiquidexProposalConsumption();
  testAmp0PsetConsumptionIfAvailable();

  console.log('ownership_matrix: all checks passed');
}

if (require.main === module) {
  try {
    runOwnershipMatrix();
  } catch (error) {
    console.error('ownership_matrix failed:', error);
    process.exitCode = 1;
  }
}

module.exports = { runOwnershipMatrix };
