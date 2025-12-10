from tokenize import Ignore
from lwk import *

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
# ANCHOR: get_utxos
utxos = wollet.utxos()
# ANCHOR_END: get_utxos

# ANCHOR: manual_coin_selection
builder = network.tx_builder()
builder.add_lbtc_recipient(node_address, sent_satoshi)
builder.set_wallet_utxos([utxos[0].outpoint()])
unsigned_pset = builder.finish(wollet)

assert len(unsigned_pset.inputs()) == 1 # ANCHOR: Ignore

signed_pset = signer.sign(unsigned_pset)
finalized_pset = wollet.finalize(signed_pset)


tx = finalized_pset.extract_tx()
# ANCHOR_END: manual_coin_selection
# Broadcast the transaction
txid = client.broadcast(tx)
wollet.wait_for_tx(txid, client)
