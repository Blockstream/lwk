from lwk import *

# Start nodes
node = LwkTestEnv()

network = Network.regtest_default()
policy_asset = network.policy_asset()
client = ElectrumClient.from_url(node.electrum_url())

# Wallet 1
s1 = Signer(Mnemonic.from_random(12), network)
w1 = Wollet(network, s1.wpkh_slip77_descriptor(), datadir=None)

# Wallet 2
s2 = Signer(Mnemonic.from_random(12), network)
w2 = Wollet(network, s2.wpkh_slip77_descriptor(), datadir=None)

# Fund both wallets
sats = 100_000
txid1 = node.send_to_address(w1.address(0).address(), sats, asset=None)
txid2 = node.send_to_address(w2.address(0).address(), sats, asset=None)
w1.wait_for_tx(txid1, client)
w2.wait_for_tx(txid2, client)

# Get the utxo from w2
utxo_w2 = w2.utxos()[0];
utxo_w2 = ExternalUtxo(
    utxo_w2.outpoint().vout(),
    w2.transactions()[0].tx(),
    utxo_w2.unblinded(),
    w2.max_weight_to_satisfy(),
    w2.is_segwit()
)

node_addr = node.get_new_address()
# Create speding tx sending all to the node
builder = network.tx_builder()
builder.add_external_utxos([utxo_w2])
builder.drain_lbtc_wallet()
builder.drain_lbtc_to(node_addr)
pset = builder.finish(w1)

pset = s1.sign(pset)

# Add the details for Wallet 2 so that Signer 2 is able to sign
pset = w2.add_details(pset)
pset = s2.sign(pset)

pset = w1.finalize(pset)
tx = pset.extract_tx()
txid = client.broadcast(tx)
w1.wait_for_tx(txid, client)
w2.wait_for_tx(txid, client)

assert w1.balance().get(policy_asset, 0) == 0
assert w2.balance().get(policy_asset, 0) == 0
assert len(w1.transactions()) == 2
assert len(w2.transactions()) == 2
