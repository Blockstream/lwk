from lwk import *

# Start nodes
node = LwkTestEnv()

# Create wallet
mnemonic = Mnemonic("abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about")
network = Network.regtest_default()
policy_asset = network.policy_asset()
client = ElectrumClient(node.electrum_url(), tls=False, validate_domain=False)

signer = Signer(mnemonic, network)
desc = signer.wpkh_slip77_descriptor()
wollet = Wollet(network, desc, datadir=None)

# Fund wallet with 2 utxos
funded_satoshi = 100000
txid = node.send_to_address(wollet.address(0).address(), funded_satoshi, asset=None)
wollet.wait_for_tx(txid, client)
txid = node.send_to_address(wollet.address(1).address(), funded_satoshi, asset=None)
wollet.wait_for_tx(txid, client)

# Create and sign a transaction selecting only 1 utxo, without manual coin selection they would be 2
address = wollet.address(1)
sent_satoshi = 1000
node_address = node.get_new_address()
utxos = wollet.utxos()

builder = network.tx_builder()
builder.add_lbtc_recipient(node_address, sent_satoshi)
builder.set_wallet_utxos([utxos[0].outpoint()])
unsigned_pset = builder.finish(wollet)

assert len(unsigned_pset.inputs()) == 1

signed_pset = signer.sign(unsigned_pset)
finalized_pset = wollet.finalize(signed_pset)

# Broadcast the transaction
tx = finalized_pset.extract_tx()
txid = client.broadcast(tx)
wollet.wait_for_tx(txid, client)
