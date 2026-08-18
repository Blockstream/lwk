from lwk import *

node = LwkTestEnv()

network = Network.regtest_default()
policy_asset = network.policy_asset()
client = ElectrumClient.from_url(node.electrum_url())

signer = Signer(Mnemonic.from_random(12), network)
wollet = Wollet(network, signer.wpkh_slip77_descriptor(), datadir=None)

sats = 100_000
txid = node.send_to_address(wollet.address(0).address(), sats, asset=None)
wollet.wait_for_tx(txid, client)

data = b"hello lwk"
b = network.tx_builder()
b.add_op_return(data)
b.add_lbtc_recipient(node.get_new_address(), 1000)
pset = b.finish(wollet)


pset = signer.sign(pset)
pset = wollet.finalize(pset)
tx = pset.extract_tx()
txid = client.broadcast(tx)
wollet.wait_for_tx(txid, client)
assert any(o.script_pubkey().bytes()[2:] == data for o in pset.outputs())
assert any(o.script_pubkey().bytes()[2:] == data for o in tx.outputs())
