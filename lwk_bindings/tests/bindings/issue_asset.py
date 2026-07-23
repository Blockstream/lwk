from lwk import *

# Start nodes
node = LwkTestEnv()

# ANCHOR: test_issue_asset
network = Network.regtest_default()
policy_asset = network.policy_asset()
client = ElectrumClient.from_url(node.electrum_url())

# Create wallet
mnemonic = Mnemonic("abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about")

signer = Signer(mnemonic, network)
desc = signer.wpkh_slip77_descriptor()
assert(str(desc) == "ct(slip77(9c8e4f05c7711a98c838be228bcb84924d4570ca53f35fa1c793e58841d47023),elwpkh([73c5da0a/84'/1'/0']tpubDC8msFGeGuwnKG9Upg7DM2b4DaRqg3CUZa5g8v2SRQ6K4NSkxUgd7HsL2XVWbVm39yBA4LAxysQAm397zwQSQoQgewGiYZqrA9DsP4zbQ1M/<0;1>/*))#2e4n992d") # ANCHOR: ignore

wollet = Wollet(network, desc, datadir=None)
wollet_address_result = wollet.address(0)
assert(wollet_address_result.index() == 0) # ANCHOR: ignore
wollet_adddress = wollet_address_result.address()
assert(str(wollet_adddress) == "el1qq2xvpcvfup5j8zscjq05u2wxxjcyewk7979f3mmz5l7uw5pqmx6xf5xy50hsn6vhkm5euwt72x878eq6zxx2z0z676mna6kdq") # ANCHOR: ignore

funded_satoshi = 100000 # ANCHOR: ignore
txid = node.send_to_address(wollet_address_result.address(), funded_satoshi, asset=None) # ANCHOR: ignore
wollet.wait_for_tx(txid, client) # ANCHOR: ignore
assert(wollet.balance()[policy_asset] == funded_satoshi) # ANCHOR: ignore

# ANCHOR: contract
contract = Contract(
    domain = "ciao.it", \
    issuer_pubkey = "0337cceec0beea0232ebe14cba0197a9fbd45fcf2ec946749de920e71434c2b904", \
    name = "name", \
    precision = 8, 
    ticker = "TTT", 
    version = 0)
assert(str(contract) == '{"entity":{"domain":"ciao.it"},"issuer_pubkey":"0337cceec0beea0232ebe14cba0197a9fbd45fcf2ec946749de920e71434c2b904","name":"name","precision":8,"ticker":"TTT","version":0}') # ANCHOR: ignore
# ANCHOR_END: contract

# ANCHOR: issue_asset
issued_asset = 10000
reissuance_tokens = 1

# Create an issuance transaction 
builder = network.tx_builder()
builder.issue_asset(issued_asset, wollet_adddress, reissuance_tokens, wollet_adddress, contract)
unsigned_pset = builder.finish(wollet)
# ANCHOR_END: issue_asset
# Sign the transaction and finalize it
signed_pset = signer.sign(unsigned_pset)
finalized_pset = wollet.finalize(signed_pset)
tx = finalized_pset.extract_tx()

# Broadcast the transaction
txid = client.broadcast(tx)

# ANCHOR: issuance_ids
asset_id = signed_pset.inputs()[0].issuance_asset()
token_id = signed_pset.inputs()[0].issuance_token()
# ANCHOR_END: issuance_ids
# ANCHOR_END: test_issue_asset
txin = tx.inputs()[0]
assert derive_asset_id(txin, contract) == asset_id
assert derive_token_id(txin, contract) == token_id

issuance = unsigned_pset.inputs()[0].issuance()
assert issuance.asset() == asset_id
assert issuance.token() == token_id
assert not issuance.is_confidential()
assert not issuance.is_null()
assert issuance.is_issuance()
assert not issuance.is_reissuance()
assert issuance.asset_satoshi() == issued_asset
assert issuance.token_satoshi() == reissuance_tokens

wollet.wait_for_tx(txid, client)

assert(wollet.balance()[asset_id] == issued_asset)
assert(wollet.balance()[token_id] == reissuance_tokens)

# ANCHOR: reissue_asset
reissue_asset = 100
asset_receiver = None  # Send the asset to the wollet creating the PSET
issuance_tx = None # issunce transaction is present in the same wallet
builder = network.tx_builder()
builder.reissue_asset(asset_id, reissue_asset, asset_receiver, issuance_tx)
unsigned_pset = builder.finish(wollet)
signed_pset = signer.sign(unsigned_pset)
finalized_pset = wollet.finalize(signed_pset)
tx = finalized_pset.extract_tx()
txid = client.broadcast(tx)
# ANCHOR_END: reissue_asset

reissuance = next(e.issuance() for e in unsigned_pset.inputs() if e.issuance())
assert reissuance.asset() == asset_id
assert reissuance.token() == token_id
assert not reissuance.is_confidential()
assert not reissuance.is_null()
assert not reissuance.is_issuance()
assert reissuance.is_reissuance()
assert reissuance.asset_satoshi() == reissue_asset
assert reissuance.token_satoshi() is None

wollet.wait_for_tx(txid, client)

assert(wollet.balance()[asset_id] == issued_asset + reissue_asset)

# ANCHOR: burn_asset
burn_asset = 50
builder = network.tx_builder()
builder.add_burn(burn_asset, asset_id)
unsigned_pset = builder.finish(wollet)
signed_pset = signer.sign(unsigned_pset)
finalized_pset = wollet.finalize(signed_pset)
tx = finalized_pset.extract_tx()
txid = client.broadcast(tx)
# ANCHOR_END: burn_asset

wollet.wait_for_tx(txid, client)

assert(wollet.balance()[asset_id] == issued_asset + reissue_asset - burn_asset)

# Issue two assets in the same transaction
txid = node.send_to_address(wollet.address(2).address(), funded_satoshi, asset=None)
wollet.wait_for_tx(txid, client)

lbtc_utxos = [u for u in wollet.utxos() if u.unblinded().asset() == policy_asset]
assert len(lbtc_utxos) == 2

builder = network.tx_builder()

asset_receiver0 = wollet.address(4).address()
token_receiver0 = wollet.address(5).address()
request0 = IssuanceRequest(30, 3)
request0.address_asset(asset_receiver0)
request0.address_token(token_receiver0)
builder.add_issuance(request0)

asset_receiver1 = wollet.address(6).address()
request1 = IssuanceRequest(40, 4)
request1.address_asset(asset_receiver1)
request1.contract(contract)
builder.add_issuance(request1)

builder.set_wallet_utxos([u.outpoint() for u in lbtc_utxos])
unsigned_pset = builder.finish(wollet)

assert len(unsigned_pset.inputs()) == 2
issuance0 = unsigned_pset.inputs()[0].issuance()
issuance1 = unsigned_pset.inputs()[1].issuance()
assert issuance0.asset_satoshi() == 30
assert issuance0.token_satoshi() == 3
assert issuance1.asset_satoshi() == 40
assert issuance1.token_satoshi() == 4

multi_asset0, multi_token0 = issuance0.asset(), issuance0.token()
multi_asset1, multi_token1 = issuance1.asset(), issuance1.token()
assert multi_asset0 != multi_asset1

signed_pset = signer.sign(unsigned_pset)
finalized_pset = wollet.finalize(signed_pset)
tx = finalized_pset.extract_tx()
txid = client.broadcast(tx)
wollet.wait_for_tx(txid, client)

assert(wollet.balance()[multi_asset0] == 30)
assert(wollet.balance()[multi_token0] == 3)
assert(wollet.balance()[multi_asset1] == 40)
assert(wollet.balance()[multi_token1] == 4)

# Pin two issuances, each to a different input, in an explicit inputs order
txid = node.send_to_address(wollet.address(3).address(), funded_satoshi, asset=None)
wollet.wait_for_tx(txid, client)

lbtc_utxos = [u for u in wollet.utxos() if u.unblinded().asset() == policy_asset]
assert len(lbtc_utxos) == 2
first_outpoint = lbtc_utxos[0].outpoint()
second_outpoint = lbtc_utxos[1].outpoint()

# ANCHOR: pin_input
request0 = IssuanceRequest(50, 5)
request0.pin_input(first_outpoint)
request1 = IssuanceRequest(60, 6)
request1.pin_input(second_outpoint)

builder = network.tx_builder()
builder.set_wallet_utxos([first_outpoint, second_outpoint])
builder.set_inputs_order([first_outpoint, second_outpoint])
builder.add_issuance(request0)
builder.add_issuance(request1)
unsigned_pset = builder.finish(wollet)
# ANCHOR_END: pin_input

pinned_inputs = unsigned_pset.inputs()
assert len(pinned_inputs) == 2
assert str(pinned_inputs[0].previous_txid()) == str(first_outpoint.txid())
assert pinned_inputs[0].previous_vout() == first_outpoint.vout()
assert str(pinned_inputs[1].previous_txid()) == str(second_outpoint.txid())
assert pinned_inputs[1].previous_vout() == second_outpoint.vout()

pinned_issuance0 = pinned_inputs[0].issuance()
pinned_issuance1 = pinned_inputs[1].issuance()
assert pinned_issuance0.asset_satoshi() == 50
assert pinned_issuance0.token_satoshi() == 5
assert pinned_issuance1.asset_satoshi() == 60
assert pinned_issuance1.token_satoshi() == 6

pinned_asset0, pinned_token0 = pinned_issuance0.asset(), pinned_issuance0.token()
pinned_asset1, pinned_token1 = pinned_issuance1.asset(), pinned_issuance1.token()
assert pinned_asset0 != pinned_asset1

signed_pset = signer.sign(unsigned_pset)
finalized_pset = wollet.finalize(signed_pset)
tx = finalized_pset.extract_tx()
txid = client.broadcast(tx)
wollet.wait_for_tx(txid, client)

assert(wollet.balance()[pinned_asset0] == 50)
assert(wollet.balance()[pinned_token0] == 5)
assert(wollet.balance()[pinned_asset1] == 60)
assert(wollet.balance()[pinned_token1] == 6)

