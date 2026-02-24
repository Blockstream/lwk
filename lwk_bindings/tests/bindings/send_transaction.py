import os

from lwk import *


_TX_HEX_PATH = os.path.join(os.path.dirname(__file__), "..", "test_data", "tx.hex")
with open(_TX_HEX_PATH) as f:
    TEST_TX_HEX = f.read().strip()

tx_from_str = Transaction.from_string(TEST_TX_HEX)
assert str(tx_from_str) == TEST_TX_HEX

tx_bytes = tx_from_str.to_bytes()
tx_from_bytes = Transaction.from_bytes(tx_bytes)

assert tx_from_bytes.to_bytes() == tx_bytes
assert str(tx_from_bytes) == TEST_TX_HEX
assert str(tx_from_str.txid()) == str(tx_from_bytes.txid())

node = LwkTestEnv() # launch electrs and elementsd


mnemonic = Mnemonic("abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about")
network = Network.regtest_default()
policy_asset = network.policy_asset()
client = ElectrumClient.from_url(node.electrum_url())

signer = Signer(mnemonic, network)
desc = signer.wpkh_slip77_descriptor()

assert(str(desc) == "ct(slip77(9c8e4f05c7711a98c838be228bcb84924d4570ca53f35fa1c793e58841d47023),elwpkh([73c5da0a/84'/1'/0']tpubDC8msFGeGuwnKG9Upg7DM2b4DaRqg3CUZa5g8v2SRQ6K4NSkxUgd7HsL2XVWbVm39yBA4LAxysQAm397zwQSQoQgewGiYZqrA9DsP4zbQ1M/<0;1>/*))#2e4n992d")

wollet = Wollet(network, desc, datadir=None)
wollet_address = wollet.address(0)
assert(wollet_address.index() == 0)
assert(str(wollet_address.address()) == "el1qq2xvpcvfup5j8zscjq05u2wxxjcyewk7979f3mmz5l7uw5pqmx6xf5xy50hsn6vhkm5euwt72x878eq6zxx2z0z676mna6kdq")

funded_satoshi = 100000
txid = node.send_to_address(wollet_address.address(), funded_satoshi, asset=None)
wollet.wait_for_tx(txid, client)

assert(wollet.balance()[policy_asset] == funded_satoshi)

node_address = node.get_new_address()
sent_satoshi = 1000

builder = network.tx_builder()
builder.add_lbtc_recipient(node_address, sent_satoshi)
unsigned_pset = builder.finish(wollet)
signed_pset = signer.sign(unsigned_pset)

# It's possible to finalize a PSET from the PSET itself, or using the wollet
tx_ = signed_pset.finalize()
finalized_pset = wollet.finalize(signed_pset)
tx = finalized_pset.extract_tx()
assert str(tx) == str(tx_)
txid = client.broadcast(tx)

wollet.wait_for_tx(txid, client)
expected_balance = funded_satoshi- sent_satoshi - tx.fee(policy_asset)
assert(wollet.balance()[policy_asset] == expected_balance)

# Create a new wallet
signer2 = Signer.random(network)
assert str(signer2.mnemonic()) != str(mnemonic)

desc2 = signer2.wpkh_slip77_descriptor()
wollet2 = Wollet(network, desc2, datadir=None)
address2 = wollet2.address(None).address()

builder = network.tx_builder()
builder.add_lbtc_recipient(address2, funded_satoshi + 1)
try:
    builder.finish(wollet)
except LwkError as e:
    assert "InsufficientFunds" in str(e), str(e)
else:
    assert False, "Should have thrown error"
