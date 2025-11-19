from lwk import *

node = LwkTestEnv() # launch electrs and elementsd

# (maker) Create a new wallet with LBTC
mnemonic = Mnemonic("abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about")
network = Network.regtest_default()
policy_asset = network.policy_asset()
client = ElectrumClient.from_url(node.electrum_url())
signer = Signer(mnemonic, network)
desc = signer.wpkh_slip77_descriptor()
maker = Wollet(network, desc, datadir=None)
utxo_size = 100000
txid = node.send_to_address(maker.address(0).address(), utxo_size, asset=None)
maker.wait_for_tx(txid, client)
assert(maker.balance()[policy_asset] == utxo_size)

# (taker) Create a second wallet with an issued asset and some LBTC required for the fee (will be the liquidex taker)
signer2 = Signer.random(network)
desc2 = signer2.wpkh_slip77_descriptor()
taker = Wollet(network, desc2, datadir=None)
address2 = taker.address(1).address()
issued_asset_units = 100
asset = node.issue_asset(issued_asset_units)
txid2 = node.send_to_address(address2, issued_asset_units, asset)
txid_fee = node.send_to_address(taker.address(1).address(), 1000, asset=None)
taker.wait_for_tx(txid2, client)
taker.wait_for_tx(txid_fee, client)
assert(taker.balance()[asset] == issued_asset_units)

# (maker) Create a liquidex proposal (asking for the issued asset in exchange for the policy asset)
builder = network.tx_builder()
utxo = maker.utxos()[0].outpoint()
builder.liquidex_make(utxo, maker.address(None).address(), issued_asset_units, asset)
pset = builder.finish(maker)
assert pset.inputs()[0].sighash() == 131
signed_pset = signer.sign(pset)

# (maker) Create the proposal  and convert it to string to pass it to the taker
proposal = UnvalidatedLiquidexProposal.from_pset(signed_pset)
proposal_str = str(proposal)

# (taker) Parse the proposal from string and validate it
proposal_from_str = UnvalidatedLiquidexProposal(proposal_str)
txid = proposal_from_str.needed_tx()
previous_tx = client.get_tx(txid)
validated_proposal = proposal_from_str.validate(previous_tx)

# (taker) Verify proposal details
input_amount = validated_proposal.input().amount()
output_amount = validated_proposal.output().amount()
input_asset = validated_proposal.input().asset()
output_asset = validated_proposal.output().asset()
assert input_amount == utxo_size
assert output_amount == issued_asset_units
assert input_asset == policy_asset
assert output_asset == asset

# (taker) Accept the proposal
builder2 = network.tx_builder()
builder2.liquidex_take([validated_proposal])
pset2 = builder2.finish(taker)
assert pset2.inputs()[1].sighash() == 1
signed_pset2 = signer2.sign(pset2)

# (taker) Finalize and broadcast the transaction
finalized_pset = taker.finalize(signed_pset2)
tx = finalized_pset.extract_tx()
txid = client.broadcast(tx)
maker.wait_for_tx(txid, client)
taker.wait_for_tx(txid, client)

# Verify the final balance
assert(maker.balance()[asset] == issued_asset_units)   
assert asset not in taker.balance()

