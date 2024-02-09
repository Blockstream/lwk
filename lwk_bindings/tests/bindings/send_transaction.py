from lwk_bindings import *

node = TestEnv() # launch electrs and elementsd


mnemonic = Mnemonic("abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about")
network = Network.regtest_default()
policy_asset = network.policy_asset()
client = ElectrumClient(node.electrum_url(), False, False)

signer = Signer(mnemonic, network)
desc = signer.wpkh_slip77_descriptor()

assert(str(desc) == "ct(slip77(9c8e4f05c7711a98c838be228bcb84924d4570ca53f35fa1c793e58841d47023),elwpkh([73c5da0a/84'/1'/0']tpubDC8msFGeGuwnKG9Upg7DM2b4DaRqg3CUZa5g8v2SRQ6K4NSkxUgd7HsL2XVWbVm39yBA4LAxysQAm397zwQSQoQgewGiYZqrA9DsP4zbQ1M/<0;1>/*))#2e4n992d")

wollet = Wollet(network, desc, None)
wollet_address = wollet.address(0)
assert(wollet_address.index() == 0)
assert(str(wollet_address.address()) == "el1qq2xvpcvfup5j8zscjq05u2wxxjcyewk7979f3mmz5l7uw5pqmx6xf5xy50hsn6vhkm5euwt72x878eq6zxx2z0z676mna6kdq")

funded_satoshi = 100000
txid = node.sendtoaddress(wollet_address.address(), funded_satoshi, None)
wollet.wait_for_tx(txid, client)

assert(wollet.balance()[policy_asset] == funded_satoshi)

node_address = node.getnewaddress()
sent_satoshi = 1000
unsigned_pset = wollet.send_lbtc(sent_satoshi, node_address, 100.0 )
signed_pset = signer.sign(unsigned_pset)
finalized_pset = wollet.finalize(signed_pset)
tx = finalized_pset.extract_tx()
txid = client.broadcast(tx)

wollet.wait_for_tx(txid, client)
expected_balance = funded_satoshi- sent_satoshi - tx.fee(policy_asset)
assert(wollet.balance()[policy_asset] == expected_balance)
