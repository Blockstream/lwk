from lwk import *

node = LwkTestEnv() # launch electrs and elementsd

network = Network.regtest_default()

# Create signers
mnemonic1 = Mnemonic.from_random(12)
mnemonic2 = Mnemonic.from_random(12)

s1 = Signer(mnemonic1, network)
s2 = Signer(mnemonic2, network)
xpub1 = s1.keyorigin_xpub(Bip.new_bip87())
xpub2 = s2.keyorigin_xpub(Bip.new_bip87())

# Create wallets
client = ElectrumClient.from_url(node.electrum_url())

# Multisig
desc = f"ct(elip151,elwsh(multi(2,{xpub1}/<0;1>/*,{xpub2}/<0;1>/*)))"
desc = WolletDescriptor(desc)
wm = Wollet(network, desc, datadir=None)

# Singlesig
desc = s1.wpkh_slip77_descriptor()
ws = Wollet(network, desc, datadir=None)

# Fund wallets
sats = 100000
txid = node.send_to_address(wm.address(0).address(), sats, None)
wm.wait_for_tx(txid, client)

# Issue asset
# Multisig wallet pays fees and receives token
# Singlesig receives asset
asset_sats, token_sats = 10, 1
contract = Contract(
    domain="liquidtestnet.com",
    issuer_pubkey="020202020202020202020202020202020202020202020202020202020202020202",
    name="name",
    precision=8,
    ticker="CRT",
    version=0
)

b = network.tx_builder()
b.issue_asset(
    asset_sats,
    ws.address(0).address(),
    token_sats,
    wm.address(0).address(),
    contract
)
pset = b.finish(wm)

pset = s1.sign(pset)
pset = s2.sign(pset)
pset = wm.finalize(pset)
tx = pset.extract_tx()
txid = client.broadcast(tx)
ws.wait_for_tx(txid, client)
wm.wait_for_tx(txid, client)

asset_id = pset.inputs()[0].issuance_asset()
token_id = pset.inputs()[0].issuance_token()

assert(ws.balance()[asset_id] == asset_sats)
assert(wm.balance()[token_id] == token_sats)

# Reissue more than the 21m "bitcoin" by doing a chain of reissuances
reissue_sats = 2100000000000000

n_reissuances = 3

# Create 1st PSET
b = network.tx_builder()
b.reissue_asset(asset_id, reissue_sats, ws.address(0).address(), None)
pset = b.finish(wm)
psets = [pset]

for _ in range(1, n_reissuances):
    # Following PSETs spend the utxos created by the previous transaction.
    previous_utxos = wm.extract_wallet_utxos(psets[-1])
    b = network.tx_builder()
    b.add_external_utxos(previous_utxos)
    # Prevent tx builder from adding more wallet utxos, those have been
    # added in the 1st PSET so they would be double spent here.
    b.set_wallet_utxos([])
    b.reissue_asset(asset_id, reissue_sats, ws.address(0).address(), None)
    pset = b.finish(wm)
    psets.append(pset)

# Signer 1 signs all PSETs
psets = [s1.sign(pset) for pset in psets]

# Signer 2 signs all PSETs
psets = [s2.sign(pset) for pset in psets]

# Broadcast all PSETs
for pset in psets:
    txid = client.broadcast(wm.finalize(pset).extract_tx())
    ws.wait_for_tx(txid, client)
    wm.wait_for_tx(txid, client)

assert(ws.balance()[asset_id] == asset_sats + len(psets) * reissue_sats)
assert(wm.balance()[token_id] == token_sats)
