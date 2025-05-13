from lwk import *

node = TestEnv() # launch electrs and elementsd

# Create a new wallet with LBTC (will be the liquidex maker)
mnemonic = Mnemonic("abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about")
network = Network.regtest_default()
policy_asset = network.policy_asset()
client = ElectrumClient(node.electrum_url(), tls=False, validate_domain=False)
signer = Signer(mnemonic, network)
desc = signer.wpkh_slip77_descriptor()
wollet = Wollet(network, desc, datadir=None)
utxo_size = 100000
txid = node.send_to_address(wollet.address(0).address(), utxo_size, asset=None)
wollet.wait_for_tx(txid, client)
assert(wollet.balance()[policy_asset] == utxo_size)

# Create a second wallet with an issued asset and some LBTC required for the fee (will be the liquidex taker)
signer2 = Signer.random(network)
desc2 = signer2.wpkh_slip77_descriptor()
wollet2 = Wollet(network, desc2, datadir=None)
address2 = wollet2.address(1).address()
issued_asset_units = 100
asset = node.issue_asset(issued_asset_units)
txid2 = node.send_to_address(address2, issued_asset_units, asset)
txid_fee = node.send_to_address(wollet2.address(1).address(), 1000, asset=None)
wollet2.wait_for_tx(txid2, client)
wollet2.wait_for_tx(txid_fee, client)
assert(wollet2.balance()[asset] == issued_asset_units)

# Create a liquidex proposal (asking for the issued asset in exchange for the policy asset)
builder = network.tx_builder()
utxo = wollet.utxos()[0].outpoint()
builder.liquidex_make(utxo, wollet.address(None).address(), issued_asset_units, asset)
pset = builder.finish(wollet)
signed_pset = signer.sign(pset)

# Create and validate the proposal
proposal = UnvalidatedLiquidexProposal.from_pset(signed_pset)
validated_proposal = proposal.insecure_validate()

# Verify proposal details
input_amount = validated_proposal.input().amount()
output_amount = validated_proposal.output().amount()
input_asset = validated_proposal.input().asset()
output_asset = validated_proposal.output().asset()

assert input_amount == utxo_size
assert output_amount == issued_asset_units
assert input_asset == policy_asset
assert output_asset == asset

# Have the taker accept the proposal
builder2 = network.tx_builder()
builder2.liquidex_take([validated_proposal])
pset2 = builder2.finish(wollet2)
signed_pset2 = signer2.sign(pset2)

# Finalize and broadcast the transaction
finalized_pset = wollet2.finalize(signed_pset2)
tx = finalized_pset.extract_tx()
txid = client.broadcast(tx)
wollet.wait_for_tx(txid, client)
wollet2.wait_for_tx(txid, client)

# Verify the final balance
assert(wollet.balance()[asset] == issued_asset_units)   
assert asset not in wollet2.balance()

