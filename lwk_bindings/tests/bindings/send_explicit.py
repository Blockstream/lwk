from lwk import *

node = LwkTestEnv()

network = Network.regtest_default()
signer = Signer(Mnemonic.from_random(12), network)
desc = signer.wpkh_slip77_descriptor()
wollet = Wollet(network, desc, datadir=None)

policy_asset = network.policy_asset()
client = ElectrumClient.from_url(node.electrum_url())

addr = wollet.address(0).address()
sats = 10000
txid = node.send_to_address(addr, sats, asset=None)
wollet.wait_for_tx(txid, client)
assert(wollet.balance()[policy_asset] == sats)

addr_explicit = node.get_new_address().to_unconfidential()
b = network.tx_builder()
b.add_explicit_recipient(addr_explicit, 1000, policy_asset)
pset = b.finish(wollet)
pset = signer.sign(pset)
tx = pset.finalize()
txid = client.broadcast(tx)
wollet.wait_for_tx(txid, client)

fee = tx.fee(policy_asset)

assert(wollet.balance()[policy_asset] == sats - 1000 - fee)
