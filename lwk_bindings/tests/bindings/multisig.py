from lwk import *

node = LwkTestEnv() # launch electrs and elementsd

# ANCHOR: multisig-setup
network = Network.testnet()
network = Network.regtest_default() # ANCHOR: ignore
# Derivation for multisig
bip = Bip.new_bip87()

# Alice creates their signer and gets the xpub
signer_a = Signer.random(network)
xpub_a = signer_a.keyorigin_xpub(bip);

# Bob creates their signer and gets the xpub
signer_b = Signer.random(network)
xpub_b = signer_b.keyorigin_xpub(bip);

# Carol, who acts as a coordinator, creates their signer and gets the xpub
signer_c = Signer.random(network)
xpub_c = signer_c.keyorigin_xpub(bip);

# Carol generates a random SLIP77 descriptor blinding key
import os
slip77_rand_key = os.urandom(32).hex()
desc_blinding_key = f"slip77({slip77_rand_key})"

# Carol uses the collected xpubs and the descriptor blinding key to create
# the 2of3 descriptor
threshold = 2;
desc = f"ct({desc_blinding_key},elwsh(multi({threshold},{xpub_a}/<0;1>/*,{xpub_b}/<0;1>/*,{xpub_c}/<0;1>/*)))"
# Validate the descriptor string
wd = WolletDescriptor(desc)
# ANCHOR_END: multisig-setup

# ANCHOR: multisig-receive
# Carol creates the wollet
wollet_c = Wollet(network, wd, datadir=None)

# With the wollet, Carol can obtain addresses, transactions and balance
addr = wollet_c.address(None);
txs = wollet_c.transactions();
balance = wollet_c.balance();

# Update the wollet state
url = "https://blockstream.info/liquidtestnet/api"
client = EsploraClient(url, network)
client = ElectrumClient(node.electrum_url(), tls=False, validate_domain=False)  # ANCHOR: ignore

update = client.full_scan(wollet_c)
wollet_c.apply_update(update)
# ANCHOR_END: multisig-receive

# Receive some funds
client = ElectrumClient(node.electrum_url(), tls=False, validate_domain=False)
txid = node.send_to_address(wollet_c.address(0).address(), 10_000, asset=None)
wollet_c.wait_for_tx(txid, client)
