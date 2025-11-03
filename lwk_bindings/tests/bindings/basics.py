from lwk import *

node = LwkTestEnv() # launch electrs and elementsd

# ANCHOR: generate-signer
mnemonic = Mnemonic.from_random(12)
network = Network.testnet()
network = Network.regtest_default()  # ANCHOR: ignore
signer = Signer(mnemonic, network)
# ANCHOR_END: generate-signer

# ANCHOR: get-xpub
xpub = signer.keyorigin_xpub(Bip.new_bip84())
# ANCHOR_END: get-xpub

# ANCHOR: wollet
desc = signer.wpkh_slip77_descriptor()
wollet = Wollet(network, desc, datadir=None)
# ANCHOR_END: wollet

# ANCHOR: address
addr = wollet.address(None)

# ANCHOR_END: address
client = ElectrumClient(node.electrum_url(), tls=False, validate_domain=False)

sats = 100000
txid = node.send_to_address(addr.address(), sats, asset=None)
wollet.wait_for_tx(txid, client)

# ANCHOR: txs
txs = wollet.transactions()
balance = wollet.balance()
# ANCHOR_END: txs

# ANCHOR: electrum_client
# Create electrum client with custom URL
client = ElectrumClient("blockstream.info:995", tls=True, validate_domain=True)

# Or use the default electrum client for the network
default_client = Network.mainnet().default_electrum_client()
# ANCHOR_END: electrum_client

# ANCHOR: esplora_client
url = "https://blockstream.info/liquid/api"
client = EsploraClient(url, Network.mainnet())
# ANCHOR_END: esplora_client

# ANCHOR: waterfalls_client
url = "https://waterfalls.liquidwebwallet.org/liquid/api"
client = EsploraClient.new_waterfalls(url, Network.mainnet())
# ANCHOR_END: waterfalls_client

# ANCHOR: client
url = "https://blockstream.info/liquidtestnet/api"
client = EsploraClient(url, network)
client = ElectrumClient(node.electrum_url(), tls=False, validate_domain=False)  # ANCHOR: ignore

update = client.full_scan(wollet)
wollet.apply_update(update)
# ANCHOR_END: client

# Receive some funds
address = node.get_new_address()
sats = 1000
lbtc = network.policy_asset()

# ANCHOR: tx
b = network.tx_builder()
b.add_recipient(address, sats, lbtc)
pset = b.finish(wollet)
# ANCHOR_END: tx

# ANCHOR: pset-details
details = wollet.pset_details(pset)
# ANCHOR_END: pset-details

# ANCHOR: sign
pset = signer.sign(pset)
# ANCHOR_END: sign

# ANCHOR: broadcast
tx = pset.finalize()
txid = client.broadcast(tx)

# (optional)
wollet.apply_transaction(tx)
# ANCHOR_END: broadcast
