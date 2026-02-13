import os
import time
from lwk import *

_SIMF_DIR = os.path.join(os.path.dirname(__file__), "..", "..", "..", "lwk_simplicity", "data")
P2PK_SOURCE = open(os.path.join(_SIMF_DIR, "p2pk.simf")).read()

# 1. Set up regtest environment
node = LwkTestEnv()
network = Network.regtest_default()
policy_asset = network.policy_asset()
client = ElectrumClient.from_url(node.electrum_url())

# 2. Create signer and derive x-only public key
mnemonic = Mnemonic("abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about")
signer = Signer(mnemonic, network)
derivation_path = "m/86'/1'/0'/0/0"
xonly_pubkey = simplicity_derive_xonly_pubkey(signer, derivation_path)

# 3. Compile P2PK program with the public key
args = SimplicityArguments()
args = args.add_value("PUBLIC_KEY", SimplicityTypedValue.u256(xonly_pubkey.to_hex()))
program = SimplicityProgram.load(P2PK_SOURCE, args)

# 4. Create P2TR address from the program
simplicity_address = program.create_p2tr_address(xonly_pubkey, network)
simplicity_script = simplicity_address.script_pubkey()

# Create Wollet
desc = WolletDescriptor(f":{simplicity_script}")
wollet = Wollet(network, desc, datadir=None)
assert str(simplicity_address) == str(wollet.address(0).address())

# 5. Fund the Simplicity address
funded_satoshi = 100000
funding_txid = node.send_to_address(simplicity_address, funded_satoshi, asset=None)
node.generate(1)

# 6. Find the funding UTXO - with retry for electrs sync
# Note: We can't use wollet.wait_for_tx() because:
#   1. The Simplicity address isn't tracked by the dummy wallet
#      (it's a script address, not derived from the wallet descriptor)
#   2. We need the raw Transaction to extract outputs, not a WalletTx
funding_tx = None
for _ in range(30):
    try:
        funding_tx = client.get_tx(funding_txid)
        break
    except LwkError:
        time.sleep(1)
assert funding_tx is not None, "Could not fetch funding transaction after 30 retries"

# Find our output by matching script_pubkey
vout = None
funding_output = None
for i, output in enumerate(funding_tx.outputs()):
    if str(output.script_pubkey()) == str(simplicity_script):
        vout = i
        funding_output = output
        break
assert vout is not None and funding_output is not None, "Could not find funding output"

# 7. Create ExternalUtxo for TxBuilder
SIMPLICITY_WITNESS_WEIGHT = 700  # FIXME(KyrylR): Conservative estimate for Simplicity witness
unblinded = TxOutSecrets.from_explicit(policy_asset, funded_satoshi)
external_utxo = ExternalUtxo(vout, funding_tx, unblinded, SIMPLICITY_WITNESS_WEIGHT, True)

# 8. Create a dummy Wollet (needed for TxBuilder.finish but won't provide UTXOs)
dummy_signer = Signer(Mnemonic.from_random(12), network)
dummy_wollet = Wollet(network, dummy_signer.wpkh_slip77_descriptor(), datadir=None)

# 9. Build transaction using TxBuilder
recipient_address = node.get_new_address()
send_amount = 50000

builder = network.tx_builder()
builder.add_external_utxos([external_utxo])
builder.add_lbtc_recipient(recipient_address, send_amount)
builder.drain_lbtc_to(simplicity_address)  # Change back to Simplicity address
pset = builder.finish(dummy_wollet)

# 10. Extract unsigned transaction and create signature
unsigned_tx = pset.extract_tx()
all_utxos = [funding_output]

signature = program.create_p2pk_signature(
    signer, derivation_path, unsigned_tx,
    all_utxos, 0, network
)

# 11. Finalize transaction with Simplicity witness
witness = SimplicityWitnessValues()
witness = witness.add_value("SIGNATURE", SimplicityTypedValue.byte_array(str(signature)))

finalized_tx = program.finalize_transaction(
    unsigned_tx, xonly_pubkey, all_utxos, 0,
    witness, network, SimplicityLogLevel.NONE
)

# 11b. Verify TxInWitness can be built manually and matches finalize_transaction output
finalized_witness = finalized_tx.inputs()[0].witness()
assert not finalized_witness.is_empty(), "Finalized witness should not be empty"
finalized_script_witness = finalized_witness.script_witness()
assert len(finalized_script_witness) == 4, "Simplicity witness should have 4 elements"

# Run the program to get the pruned program and witness bytes
run_result = program.run(
    unsigned_tx, xonly_pubkey, all_utxos, 0,
    witness, network, SimplicityLogLevel.NONE
)

# Build the witness manually from its components:
# [simplicity_witness_bytes, simplicity_program_bytes, cmr, control_block]
simplicity_witness_bytes = run_result.witness_bytes()
simplicity_program_bytes = run_result.program_bytes()
cmr = run_result.cmr()

control_block = simplicity_control_block(cmr, xonly_pubkey)
control_block_hex = control_block.serialize().hex()

# Verify it matches what program.control_block() returns
program_control_block_hex = str(program.control_block(xonly_pubkey))
assert control_block_hex == program_control_block_hex, \
    "simplicity_control_block should match program.control_block()"

manual_script_witness = [
    str(simplicity_witness_bytes),
    str(simplicity_program_bytes),
    cmr.to_hex(),
    control_block_hex,
]

manual_witness = TxInWitness.from_script_witness(manual_script_witness)
assert manual_witness.script_witness() == finalized_script_witness, \
    f"Manual witness should match finalized witness:\n  manual={manual_witness.script_witness()}\n  finalized={finalized_script_witness}"

# Test TransactionEditor.set_input_witness produces same result
tx_builder = TransactionEditor.from_transaction(unsigned_tx)
tx_builder.set_input_witness(0, manual_witness)
tx_with_manual_witness = tx_builder.build()
assert tx_with_manual_witness.inputs()[0].witness().script_witness() == finalized_script_witness, \
    "TransactionEditor.set_input_witness should produce matching witness"

# 12. Broadcast and verify inclusion in block
txid = client.broadcast(finalized_tx)
node.generate(1)

assert txid is not None
confirmed_tx = None
for _ in range(30):
    try:
        confirmed_tx = client.get_tx(txid)
        break
    except LwkError:
        time.sleep(1)
assert confirmed_tx is not None, "Transaction not confirmed in block"
