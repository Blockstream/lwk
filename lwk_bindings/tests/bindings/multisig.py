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
client = ElectrumClient.from_url(node.electrum_url())  # ANCHOR: ignore

update = client.full_scan(wollet_c)
wollet_c.apply_update(update)
# ANCHOR_END: multisig-receive

# Receive some funds
client = ElectrumClient.from_url(node.electrum_url())
txid = node.send_to_address(wollet_c.address(0).address(), 10_000, asset=None)
wollet_c.wait_for_tx(txid, client)

# ANCHOR: multisig-send
# Carol creates a transaction send few sats to a certain address
address = "<address>"
address = node.get_new_address() # ANCHOR: ignore
sats = 1000
lbtc = network.policy_asset()

b = network.tx_builder()
b.add_recipient(address, sats, lbtc)
pset = b.finish(wollet_c)

pset = signer_c.sign(pset)

# Carol sends the PSET to Bob
# Bob wants to analyze the PSET before signing, thus he creates a wollet
wd = WolletDescriptor(desc)
wollet_b = Wollet(network, wd, datadir=None)
update = client.full_scan(wollet_b)
wollet_b.apply_update(update)
# Then Bob uses the wollet to analyze the PSET
details = wollet_b.pset_details(pset)
# PSET has a reasonable fee
assert details.balance().fee() < 100
# PSET has a signature from Carol
fingerprints_has = details.fingerprints_has()
assert len(fingerprints_has) == 1
assert signer_c.fingerprint() in fingerprints_has
# PSET needs a signature from either Bob or Carol
fingerprints_missing = details.fingerprints_missing()
assert len(fingerprints_missing) == 2
assert signer_a.fingerprint() in fingerprints_missing
assert signer_b.fingerprint() in fingerprints_missing
# PSET has a single recipient, with data matching what was specified above
assert len(details.balance().recipients()) == 1
recipient = details.balance().recipients()[0]
assert str(recipient.address()) == str(address)
assert recipient.asset() == lbtc
assert recipient.value() == sats

# Bob is satisified with the PSET and signs it
pset = signer_b.sign(pset)

# Bob sends the PSET back to Carol
# Carol checks that the PSET has enough signatures
details = wollet_c.pset_details(pset)
fingerprints_has = details.fingerprints_has()
assert len(fingerprints_has) == 2

# Carol finalizes the PSET and broadcast the transaction
tx = pset.finalize()
txid = client.broadcast(tx)
# ANCHOR_END: multisig-send
