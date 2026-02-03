'use strict';

const fs = require('fs');
const path = require('path');
const lwk = require('lwk_node');

const P2PK_SOURCE = fs.readFileSync(path.join(__dirname, '../../../lwk_simplicity/data/p2pk.simf'), 'utf8');

const TEST_PUBLIC_KEY = "8a65c55726dc32b59b649ad0187eb44490de681bb02601b8d3f58c8b9fff9083";
const TEST_UNSIGNED_TX = "02000000000113226c2af4a18516258790b9c6f118afdf0bfe9cb0cf79574807ddf6bf680be80000000000ffffffff0301499a818545f6bae39fc03b637f2a4e1e64e590cac1bc3a6f6d71aa4443654c140100000000000003e800225120f8b916e58321fccfb61529245bcae76bdf0cedbcb0df2b23c2ac8f1adfed577501499a818545f6bae39fc03b637f2a4e1e64e590cac1bc3a6f6d71aa4443654c140100000000000181be00225120f8b916e58321fccfb61529245bcae76bdf0cedbcb0df2b23c2ac8f1adfed577501499a818545f6bae39fc03b637f2a4e1e64e590cac1bc3a6f6d71aa4443654c140100000000000000fa000000000000";
const TEST_UTXO_SCRIPT_PUBKEY = "5120f8b916e58321fccfb61529245bcae76bdf0cedbcb0df2b23c2ac8f1adfed5775";
const TEST_UTXO_VALUE = BigInt(100000);
const TEST_SIGNATURE = "ab5173c154e62da5e4f5d983177d7398918d61d6149f3b1bb7271d00d165391c19f86c53c5d6dcfcd50f4fc4dbf8748fbb6a1614ddacd663818bfc90ef8f818d";
const TEST_FINALIZED_TX = "02000000010113226c2af4a18516258790b9c6f118afdf0bfe9cb0cf79574807ddf6bf680be80000000000ffffffff0301499a818545f6bae39fc03b637f2a4e1e64e590cac1bc3a6f6d71aa4443654c140100000000000003e800225120f8b916e58321fccfb61529245bcae76bdf0cedbcb0df2b23c2ac8f1adfed577501499a818545f6bae39fc03b637f2a4e1e64e590cac1bc3a6f6d71aa4443654c140100000000000181be00225120f8b916e58321fccfb61529245bcae76bdf0cedbcb0df2b23c2ac8f1adfed577501499a818545f6bae39fc03b637f2a4e1e64e590cac1bc3a6f6d71aa4443654c140100000000000000fa00000000000000000440ab5173c154e62da5e4f5d983177d7398918d61d6149f3b1bb7271d00d165391c19f86c53c5d6dcfcd50f4fc4dbf8748fbb6a1614ddacd663818bfc90ef8f818d76e06922d314cb8aae4db8656b36c935a030fd688921bcd037604c0371a7eb19173fff21060a15c38735d772f07a13a9e8c81c672ff1b725794653e7b2a65b5569ecb5da314478a8c43860188599c502d8d8c399c5510915d0998008ab858317852c27b030a85d612fd7152321658700adbbf004300c4020b685a4424842507d7d747e6611a740d8c421038e9744e75d423d0e2e9f164d0221bf8a65c55726dc32b59b649ad0187eb44490de681bb02601b8d3f58c8b9fff908300000000000000";

const TEST_CMR = "b685a4424842507d7d747e6611a740d8c421038e9744e75d423d0e2e9f164d02";
const TEST_ADDRESS = "tex1plzu3devry87vlds49yj9hjh8d00semdukr0jkg7z4j834hld2a6s6y4amk";

function assertEqual(actual, expected, message) {
  if (actual !== expected) {
    throw new Error(`${message}: expected "${expected}", got "${actual}"`);
  }
}

function assertNotNull(value, message) {
  if (value === null || value === undefined) {
    throw new Error(`${message}: value is null or undefined`);
  }
}

async function runSimplicityP2pkTest() {
  try {
    const network = lwk.Network.testnet();
    const genesisHash = network.genesisBlockHash();

    assertEqual(genesisHash, "a771da8e52ee6ad581ed1e9a99825e5b3b7992225534eaa2ae23244fe26ab1c1", "Genesis hash mismatch");
    assertEqual(genesisHash.length, 64, "Genesis hash length should be 64");

    // Test loading p2pk program with public key argument
    let args = new lwk.SimplicityArguments();
    args = args.addValue("PUBLIC_KEY", lwk.SimplicityTypedValue.fromU256Hex(TEST_PUBLIC_KEY));

    const program = new lwk.SimplicityProgram(P2PK_SOURCE, args);
    const cmr = lwk.bytesToHex(program.cmr());
    assertEqual(cmr, TEST_CMR, "CMR mismatch");

    // Test creating P2TR address for p2pk program
    const address = program.createP2trAddress(new lwk.XOnlyPublicKey(TEST_PUBLIC_KEY), network);
    assertEqual(address.toString(), TEST_ADDRESS, "Address mismatch");

    // Test building witness values with signature (64 bytes)
    let witness = new lwk.SimplicityWitnessValues();
    witness = witness.addValue("SIGNATURE", lwk.SimplicityTypedValue.fromByteArrayHex(TEST_SIGNATURE));

    // Test creating TxOut from explicit values
    const utxoScript = new lwk.Script(TEST_UTXO_SCRIPT_PUBKEY);
    const utxo = lwk.TxOut.fromExplicit(utxoScript, network.policyAsset(), TEST_UTXO_VALUE);
    assertEqual(utxo.value(), TEST_UTXO_VALUE, "UTXO value mismatch");

    // Test full transaction finalization with real test vectors
    const tx = new lwk.Transaction(TEST_UNSIGNED_TX);

    const finalizedTx = program.finalizeTransaction(tx, new lwk.XOnlyPublicKey(TEST_PUBLIC_KEY), [utxo], 0, witness, network, lwk.SimplicityLogLevel.None);

    assertNotNull(finalizedTx, "Finalized transaction should not be null");
    const finalizedHex = finalizedTx.toString();
    assertEqual(finalizedHex, TEST_FINALIZED_TX, "Finalized transaction mismatch");

    // Test SimplicityType constructors
    const tEither = lwk.SimplicityType.either(lwk.SimplicityType.u32(), lwk.SimplicityType.boolean());
    const _tOption = lwk.SimplicityType.option(lwk.SimplicityType.u64());
    const _tTuple = lwk.SimplicityType.fromElements([lwk.SimplicityType.u32(), lwk.SimplicityType.u256()]);
    const _tParsed = new lwk.SimplicityType("Either<u32, bool>");

    // Test SimplicityTypedValue constructors
    const _vLeft = lwk.SimplicityTypedValue.left(lwk.SimplicityTypedValue.fromU32(42), lwk.SimplicityType.boolean());
    const _vRight = lwk.SimplicityTypedValue.right(lwk.SimplicityType.u32(), lwk.SimplicityTypedValue.fromBoolean(true));
    const _vTuple = lwk.SimplicityTypedValue.fromElements([lwk.SimplicityTypedValue.fromU32(42), lwk.SimplicityTypedValue.fromU256Hex(TEST_PUBLIC_KEY)]);
    const _vNone = lwk.SimplicityTypedValue.none(lwk.SimplicityType.u64());
    const _vSome = lwk.SimplicityTypedValue.some(lwk.SimplicityTypedValue.fromU64(BigInt(1000)));
    const _vParsed = new lwk.SimplicityTypedValue("Left(42)", tEither);

    // Test add_value on builders
    let args2 = new lwk.SimplicityArguments();
    args2 = args2.addValue("MY_PARAM", lwk.SimplicityTypedValue.fromU32(42));
    assertNotNull(args2, "args2.addValue result should not be null");
    let witness2 = new lwk.SimplicityWitnessValues();
    witness2 = witness2.addValue("MY_WITNESS", lwk.SimplicityTypedValue.left(lwk.SimplicityTypedValue.fromU32(42), lwk.SimplicityType.boolean()));
    assertNotNull(witness2, "witness2.addValue result should not be null");

    // Verify add_value works for loading a program (regression)
    let args3 = new lwk.SimplicityArguments();
    args3 = args3.addValue("PUBLIC_KEY", lwk.SimplicityTypedValue.fromU256Hex(TEST_PUBLIC_KEY));
    const program2 = new lwk.SimplicityProgram(P2PK_SOURCE, args3);
    assertEqual(lwk.bytesToHex(program2.cmr()), TEST_CMR, "Program2 CMR mismatch");
  } catch (error) {
    console.error("simplicity_p2pk test failed:", error);
    throw error;
  }
}

if (require.main === module) {
  runSimplicityP2pkTest().catch(() => {
    process.exitCode = 1;
  });
}

module.exports = {runSimplicityP2pkTest};
