import time
from lwk import *

P2PK_SOURCE = """
fn main() {
    jet::bip_0340_verify((param::PUBLIC_KEY, jet::sig_all_hash()), witness::SIGNATURE)
}
"""

# 1. Set up regtest environment
node = LwkTestEnv()
network = Network.regtest_default()
policy_asset = network.policy_asset()
genesis_hash = node.genesis_block_hash()
client = ElectrumClient.from_url(node.electrum_url())

# 2. Create signer and derive x-only public key
mnemonic = Mnemonic("abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about")
signer = Signer(mnemonic, network)
derivation_path = "m/86'/1'/0'/0/0"
xonly_pubkey = simplicity_derive_xonly_pubkey(signer, derivation_path)

# 3. Compile P2PK program with the public key
args = SimplicityArguments()
args = args.add_bytes("PUBLIC_KEY", str(xonly_pubkey))
program = simplicity_load_program(P2PK_SOURCE, args)

# 4. Create P2TR address from the program
simplicity_address = simplicity_create_p2tr_address(program, str(xonly_pubkey), network)
simplicity_script = simplicity_address.script_pubkey()

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

signature = simplicity_create_p2pk_signature(
    signer, derivation_path, unsigned_tx, program,
    all_utxos, 0, network, genesis_hash
)

# 11. Finalize transaction with Simplicity witness
witness = SimplicityWitnessValues()
witness = witness.add_bytes("SIGNATURE", str(signature))

finalized_tx = simplicity_finalize_transaction(
    unsigned_tx, program, str(xonly_pubkey), all_utxos, 0,
    witness, network, genesis_hash, SimplicityLogLevel.NONE
)

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
