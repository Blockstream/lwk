from lwk import *

# Start nodes
node = LwkTestEnv()

network = Network.regtest_default()
policy_asset = network.policy_asset()
client = ElectrumClient.from_url(node.electrum_url())

# Wallet 1
signer = Signer(Mnemonic.from_random(12), network)
wollet = Wollet(network, signer.wpkh_slip77_descriptor(), datadir=None)

# Wallet 2
external_signer = Signer(Mnemonic.from_random(12), network)
external_wollet = Wollet(network, external_signer.wpkh_slip77_descriptor(), datadir=None)

# Fund both wallets
sats_asset = 10
sats_lbtc = 100_000
asset = node.issue_asset(sats_asset)
wollet_addr = wollet.address(0).address()
external_wollet_addr = external_wollet.address(0).address()
txid1 = node.send_to_address(wollet_addr, sats_asset, asset=asset)
txid2 = node.send_to_address(external_wollet_addr, sats_lbtc, asset=None)
wollet.wait_for_tx(txid1, client)
external_wollet.wait_for_tx(txid2, client)

# Get the utxo from external_wollet
# ANCHOR: external_utxo_create
external_utxo = external_wollet.utxos()[0];
external_utxo = ExternalUtxo(
    external_utxo.outpoint().vout(),
    external_wollet.transactions()[0].tx(),
    external_utxo.unblinded(),
    external_wollet.max_weight_to_satisfy(),
    external_wollet.is_segwit()
)
# ANCHOR_END: external_utxo_create

node_addr = node.get_new_address()
# Create speding tx sending all to the node # ANCHOR: ignore
# ANCHOR: external_utxo_add
builder = network.tx_builder()
# Add external UTXO (LBTC)
builder.add_external_utxos([external_utxo])
# Send asset to the node (funded by wollet's UTXOs)
builder.add_recipient(node_addr, 1, asset)
# Send LBTC back to external wollet
builder.drain_lbtc_wallet()
builder.drain_lbtc_to(external_wollet_addr)
pset = builder.finish(wollet)
# ANCHOR_END: external_utxo_add

pset = signer.sign(pset)

# Add the details for Wallet 2 so that Signer 2 is able to sign
# ANCHOR: external_utxo_sign
pset = external_wollet.add_details(pset)
pset = external_signer.sign(pset)
# ANCHOR_END: external_utxo_sign
pset = wollet.finalize(pset)
tx = pset.extract_tx()
txid = client.broadcast(tx)
wollet.wait_for_tx(txid, client)
external_wollet.wait_for_tx(txid, client)

assert wollet.balance().get(policy_asset, 0) == 0
assert wollet.balance().get(asset, 0) == sats_asset - 1
assert external_wollet.balance().get(policy_asset, 0) > 0
assert external_wollet.balance().get(asset, 0) == 0
assert len(wollet.transactions()) == 2
assert len(external_wollet.transactions()) == 2
