from lwk import *

# Start nodes
node = LwkTestEnv()

network = Network.regtest_default()
policy_asset = network.policy_asset()
client = ElectrumClient.from_url(node.electrum_url())

signer = Signer(Mnemonic.from_random(12), network)
wollet = Wollet(network, signer.wpkh_slip77_descriptor(), datadir=None)

# Fund both wallets
sats = 100_000
txid = node.send_to_address(wollet.address(0).address(), sats, asset=None)
wollet.wait_for_tx(txid, client)

node_addr = node.get_new_address()
# ANCHOR: drain_lbtc_wallet
# Create a PSET sending all LBTC to the node address
builder = network.tx_builder()
builder.drain_lbtc_wallet()
builder.drain_lbtc_to(node_addr)
pset = builder.finish(wollet)
# ANCHOR_END: drain_lbtc_wallet

pset = signer.sign(pset)
pset = wollet.finalize(pset)
tx = pset.extract_tx()
txid = client.broadcast(tx)
wollet.wait_for_tx(txid, client)

assert wollet.balance().get(policy_asset, 0) == 0
assert len(wollet.transactions()) == 2
