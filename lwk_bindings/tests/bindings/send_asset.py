from lwk import *

node = LwkTestEnv() # launch electrs and elementsd

mnemonic = Mnemonic("abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about")
network = Network.regtest_default()
policy_asset = network.policy_asset()
client = ElectrumClient(node.electrum_url(), tls=False, validate_domain=False)

signer = Signer(mnemonic, network)
desc = signer.wpkh_slip77_descriptor()

assert(str(desc) == "ct(slip77(9c8e4f05c7711a98c838be228bcb84924d4570ca53f35fa1c793e58841d47023),elwpkh([73c5da0a/84'/1'/0']tpubDC8msFGeGuwnKG9Upg7DM2b4DaRqg3CUZa5g8v2SRQ6K4NSkxUgd7HsL2XVWbVm39yBA4LAxysQAm397zwQSQoQgewGiYZqrA9DsP4zbQ1M/<0;1>/*))#2e4n992d")

wollet = Wollet(network, desc, datadir=None)
wollet_address = wollet.address(0)
assert(wollet_address.index() == 0)
assert(str(wollet_address.address()) == "el1qq2xvpcvfup5j8zscjq05u2wxxjcyewk7979f3mmz5l7uw5pqmx6xf5xy50hsn6vhkm5euwt72x878eq6zxx2z0z676mna6kdq")
wollet_address2 = wollet.address(1)
assert(wollet_address2.index() == 1)
assert(str(wollet_address2.address()) == "el1qqv8pmjjq942l6cjq69ygtt6gvmdmhesqmzazmwfsq7zwvan4kewdqmaqzegq50r2wdltkfsw9hw20zafydz4sqljz0eqe0vhc")

issue_asset = 100000
asset = node.issue_asset(issue_asset)
txid = node.send_to_address(wollet_address.address(), issue_asset, asset)
txid2 = node.send_to_address(wollet_address.address(), 10000, asset=None) # to pay the fee in the returning tx

wollet.wait_for_tx(txid, client)
wollet.wait_for_tx(txid2, client)

assert(wollet.balance()[asset] == issue_asset)

node_address = node.get_new_address()

builder = network.tx_builder()
builder.add_recipient( node_address,issue_asset-1, asset)
unsigned_pset = builder.finish(wollet)
signed_pset = signer.sign(unsigned_pset)

finalized_pset = wollet.finalize(signed_pset)
tx = finalized_pset.extract_tx()
fee_rate = 1000 * tx.fee(policy_asset) / tx.discount_vsize()
assert fee_rate - 100 < 10
txid = client.broadcast(tx)

wollet.wait_for_tx(txid, client)
assert(wollet.balance()[asset] == 1)
