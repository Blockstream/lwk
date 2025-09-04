from lwk import *

# AMP0 credentials
username = "userleo34567"
password = "userleo34567"
# AMP ID
amp_id = ""

# BIP39 mnemonic corresponding to the AMP0 account
mnemonic = Mnemonic("idea bind tissue wood february mention unable collect expand stuff snap stock")

network = Network.testnet()
url = "https://waterfalls.liquidwebwallet.org/liquidtestnet/api"

# Create AMP0 object
amp0 = Amp0(network, username, password, amp_id)

# Get an address
addr1 = str(amp0.address(1).address())
assert addr1 == 'vjTvpDMQx3EQ2bS3pmmy7RivU3QTjGyyJFJy1Y5basdKmwpW3R4YRdsxFNT7B3bPNmJkgKCRCS63AtjR'

# Create wollet
wollet_descriptor = amp0.wollet_descriptor()
wollet = Wollet(network, wollet_descriptor, None)

# Sync the wallet
client = EsploraClient.new_waterfalls(url, network)

last_index = amp0.last_index()
assert last_index > 20
update = client.full_scan_to_index(wollet, last_index)
wollet.apply_update(update)

# Get the wallet transactions
txs = wollet.transactions()
assert len(txs) > 0

# Get the balance
balance = wollet.balance()
lbtc = network.policy_asset()
lbtc_balance = balance.get(lbtc, 0)
if lbtc_balance < 500:
    print(f"Balance is insufficient to make a transaction, send some tLBTC to {addr1}")
    quit()
