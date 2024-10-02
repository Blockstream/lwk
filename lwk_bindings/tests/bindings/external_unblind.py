from lwk import *

# Start nodes
node = TestEnv()

# Create wallet
mnemonic = Mnemonic("abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about")
network = Network.regtest_default()
policy_asset = network.policy_asset()
client = ElectrumClient(node.electrum_url(), tls=False, validate_domain=False)

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
tx = finalized_pset.extract_tx()

# TODO: "externally" unblind the PSET/transaction
#for output in tx.outputs():
#    script = output.script()
#    if len(script.bytes()) == 0:
#        # fee
#        continue
#    private_blinding_key = derive_blinding_key(desc, script)
#    secrets = output.unblind(private_blinding_key)
#    assert secrets.asset() == policy_asset

# Broadcast the transaction
txid = client.broadcast(tx)
wollet.wait_for_tx(txid, client)
