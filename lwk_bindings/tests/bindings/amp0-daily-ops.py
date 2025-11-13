from lwk import *
import os


ci_branch_name = os.environ.get("CI_COMMIT_BRANCH")
if ci_branch_name is not None:
    # We are in a CI job
    if ci_branch_name != "master":
        print("Skipping test")
        quit()


# ANCHOR: amp0-daily-ops
# Signer
mnemonic = "<mnemonic>";
# AMP0 Watch-Only credentials
username = "<username>";
password = "<password>";
mnemonic = "thrive metal cactus come oval candy medal bounce captain shock permit joke"; # ANCHOR: ignore
username = "userlwk001"; # ANCHOR: ignore
password = "userlwk001"; # ANCHOR: ignore
# AMP ID (optional)
amp_id = "";

# Create AMP0 context
network = Network.testnet()

amp0 = Amp0(network, username, password, amp_id);

# Create AMP0 Wollet
wollet_descriptor = amp0.wollet_descriptor()
wollet = Wollet(network, wollet_descriptor, None)

# Get a new address
addr = str(amp0.address(None).address());

# Update the wallet with (new) blockchain data
url = "https://waterfalls.liquidwebwallet.org/liquidtestnet/api";
client = EsploraClient.new_waterfalls(url, network)
last_index = amp0.last_index()
update = client.full_scan_to_index(wollet, last_index)
wollet.apply_update(update)

# Get balance
balance = wollet.balance()
lbtc = network.policy_asset() # ANCHOR: ignore
lbtc_balance = balance.get(lbtc, 0) # ANCHOR: ignore
if lbtc_balance < 500: # ANCHOR: ignore
    print(f"Balance is insufficient to make a transaction, send some tLBTC to {addr}") # ANCHOR: ignore
    quit() # ANCHOR: ignore

# Construct a PSET sending LBTC back to the wallet
b = network.tx_builder()
b.drain_lbtc_wallet()  # send all to self
amp0pset = b.finish_for_amp0(wollet)

# User signs the PSET
signer = Signer(Mnemonic(mnemonic), network)
pset = amp0pset.pset()
pset = signer.sign(pset)

# Reconstruct the Amp0 PSET with the PSET signed by the user
amp0pset = Amp0Pset(pset, amp0pset.blinding_nonces())

# AMP0 signs
tx = amp0.sign(amp0pset)

# Broadcast the transaction
txid = client.broadcast(tx)
# ANCHOR_END: amp0-daily-ops
print(txid)
