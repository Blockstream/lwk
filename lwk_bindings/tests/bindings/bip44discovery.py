from lwk import *

env = LwkTestEnv()  # launch electrs and elementsd
client = ElectrumClient.from_url(env.electrum_url())

mnemonic = Mnemonic.from_random(12)
network = Network.regtest_default()
signer = Signer(mnemonic, network)

# Setup: fund account 0 wpkh
desc = signer.wpkh_slip77_descriptor()
w = Wollet(network, desc, datadir=None)
txid = env.send_to_address(w.address(0).address(), 1000, asset=None)
w.wait_for_tx(txid, client)

# Perform "BIP44 account discovery"

# Get values shared by all (singlesig) descriptors
master_blinding_key = signer.slip77_master_blinding_key()  # "slip77(...)"
fingerprint = signer.fingerprint()

def derive_descriptor(account_type, account_num):
    account_path = DerivationPath.ss_path(network, account_type, account_num)
    # Get the xpub from the signer (which might be handled externally)
    keyorigin_xpub = signer.keyorigin_xpub_from_path(account_path)
    # Some signers just return the xpub, reconstruct the "keyorigin xpub" string
    account_xpub = keyorigin_xpub.split("]")[1]  # strip keyorigin
    dpk = DescriptorPublicKey(f"[{fingerprint}/{account_path}]{account_xpub}")
    return WolletDescriptor.ss_desc_from_external_signer(network, account_type, account_num, master_blinding_key, dpk)

# Account discovery
GAP_LIMIT = 20
descriptors = {}  # descriptors "cache"

# Note: account types to discover are arbitrary
# e.g. some wallets might also want to discover "tr" accounts (bip86).
for account_type in ["wpkh", "shwpkh"]:
    account_num = 0
    while True:
        # Check if we already have derived this descriptor
        d = descriptors.get((account_type, account_num))
        if d is None:
            # Interact with the signer and get the descriptor
            wd = derive_descriptor(account_type, account_num)
            # Note: in a real-life setup, batching signer interactions
            # can improve performance by reducing "signer roundtrips"
            # (e.g. get 3 xpubs at a time).
            d = {
                "descriptor": wd,
                "has_txs": False,
            }

        # Skip network calls if we already know this descriptor has transactions
        if not d["has_txs"]:
            # Cheaply check whether the descriptor has transactions using the dedicated "has_txs" call
            d["has_txs"] = client.has_txs(d["descriptor"], GAP_LIMIT)

        # Update the descriptors cache
        descriptors[(account_type, account_num)] = d

        if not d["has_txs"]:
           # This account has no transactions, stop discovery for this account type
           break

        account_num += 1

# One descriptor have been discovered
assert sum(d["has_txs"] for d in descriptors.values()) == 1
# Three descriptors have been created
assert len(descriptors) == 3
# Persist "descriptors" to improve performance of future discoveries:
# skip network calls for already discovered descriptors and
# skip "signer roundtrips" for descriptors with no txs
