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
contract = Contract(domain = "ciao.it", issuer_pubkey = "0337cceec0beea0232ebe14cba0197a9fbd45fcf2ec946749de920e71434c2b904", name = "name", precision = 8, ticker = "TTT", version = 0)
assert(str(contract) == '{"entity":{"domain":"ciao.it"},"issuer_pubkey":"0337cceec0beea0232ebe14cba0197a9fbd45fcf2ec946749de920e71434c2b904","name":"name","precision":8,"ticker":"TTT","version":0}') # ANCHOR: ignore
# ANCHOR_END: contract

# ANCHOR: issue_asset
issued_asset = 10000
reissuance_tokens = 1

# Create a transaction builder and the issuance transaction 
builder = network.tx_builder()
builder.issue_asset(issued_asset, wollet_adddress, reissuance_tokens, wollet_adddress, contract)
unsigned_pset = builder.finish(wollet)

# Sign the transaction and finalize it
signed_pset = signer.sign(unsigned_pset)
finalized_pset = wollet.finalize(signed_pset)
tx = finalized_pset.extract_tx()

# Broadcast the transaction
txid = client.broadcast(tx)
# ANCHOR_END: issue_asset

# ANCHOR: issuance_ids
asset_id = signed_pset.inputs()[0].issuance_asset()
token_id = signed_pset.inputs()[0].issuance_token()
# ANCHOR_END: issuance_ids

txin = tx.inputs()[0]
# ANCHOR_END: test_issue_asset
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

## reissue the asset
reissue_asset = 100
builder = network.tx_builder()
builder.reissue_asset(asset_id, 100, None, None)
unsigned_pset = builder.finish(wollet)
signed_pset = signer.sign(unsigned_pset)
finalized_pset = wollet.finalize(signed_pset)
tx = finalized_pset.extract_tx()
txid = client.broadcast(tx)

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


