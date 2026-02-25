from lwk import *

TEST_BLINDING_FACTOR_HEX = "0000460186f7233c927e7db2dcc703c0e500b653ca82273b7bfad8045d85a470"

abf_from_str = AssetBlindingFactor.from_string(TEST_BLINDING_FACTOR_HEX)
vbf_from_str = ValueBlindingFactor.from_string(TEST_BLINDING_FACTOR_HEX)

abf_bytes = abf_from_str.to_bytes()
abf_from_bytes = AssetBlindingFactor.from_bytes(abf_bytes)

vbf_bytes = vbf_from_str.to_bytes()
vbf_from_bytes = ValueBlindingFactor.from_bytes(vbf_bytes)

assert str(abf_from_str) == TEST_BLINDING_FACTOR_HEX
assert str(vbf_from_str) == TEST_BLINDING_FACTOR_HEX

assert abf_from_bytes.to_bytes() == abf_bytes
assert str(abf_from_bytes) == TEST_BLINDING_FACTOR_HEX

assert vbf_from_bytes.to_bytes() == vbf_bytes
assert str(vbf_from_bytes) == TEST_BLINDING_FACTOR_HEX

# Start nodes
node = LwkTestEnv()

# Create wallet
mnemonic = Mnemonic("abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about")
network = Network.regtest_default()
policy_asset = network.policy_asset()
client = ElectrumClient.from_url(node.electrum_url())

signer = Signer(mnemonic, network)
desc = signer.wpkh_slip77_descriptor()
wollet = Wollet(network, desc, datadir=None)

# Fund wallet
address = wollet.address(0)
funded_satoshi = 100000
txid = node.send_to_address(address.address(), funded_satoshi, asset=None)
wollet.wait_for_tx(txid, client)

# Create and sign a transaction
address = wollet.address(1)
sent_satoshi = 1000

builder = network.tx_builder()
builder.add_lbtc_recipient(address.address(), sent_satoshi)
unsigned_pset = builder.finish(wollet)
signed_pset = signer.sign(unsigned_pset)
finalized_pset = wollet.finalize(signed_pset)

# Check that all inputs are provably segwit
# This can be useful if we don't want that the txid changes after signatures are added/changed
for input_ in unsigned_pset.inputs():
    # You might want to check the script pubkey against the "real" utxo from the node
    script_pubkey = input_.previous_script_pubkey()
    redeem_script = input_.redeem_script()
    assert is_provably_segwit(script_pubkey, redeem_script)

# "externally" unblind the PSET/transaction
tx = finalized_pset.extract_tx()
for output in tx.outputs():
    spk = output.script_pubkey()
    if output.is_fee():
        continue
    private_blinding_key = desc.derive_blinding_key(spk)
    # Roundtrip the blinding key as caller might persist it as bytes
    private_blinding_key = SecretKey.from_bytes(private_blinding_key.bytes())
    secrets = output.unblind(private_blinding_key)
    assert secrets.asset() == policy_asset

# Broadcast the transaction
txid = client.broadcast(tx)
wollet.wait_for_tx(txid, client)
