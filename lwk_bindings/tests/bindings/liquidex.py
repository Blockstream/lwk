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

# ANCHOR: liquidex_make
# (maker) Create a liquidex proposal (asking for the issued asset in exchange for the policy asset)
builder = network.tx_builder()
utxo = maker.utxos()[0].outpoint()
builder.liquidex_make(utxo, maker.address(None).address(), issued_asset_units, asset)
pset = builder.finish(maker)
assert pset.inputs()[0].sighash() == 131 # ANCHOR: ignore
assert maker.pset_details(pset).has_non_default_sighash() # ANCHOR: ignore
options = SignOptions.allow_sighash_single_anyonecanpay()
signed_pset = signer.sign_with_options(pset, options)

# (maker) Create the proposal  and convert it to string to pass it to the taker
proposal = UnvalidatedLiquidexProposal.from_pset(signed_pset)
proposal_str = str(proposal)
# ANCHOR_END: liquidex_make

# ANCHOR: liquidex_validate
# (taker) Parse the proposal from string and validate it
proposal_from_str = UnvalidatedLiquidexProposal(proposal_str)
txid = proposal_from_str.needed_tx()
previous_tx = client.get_tx(txid)
validated_proposal = proposal_from_str.validate(previous_tx)
# ANCHOR_END: liquidex_validate
# (taker) Verify proposal details
input_amount = validated_proposal.input().amount()
output_amount = validated_proposal.output().amount()
input_asset = validated_proposal.input().asset()
output_asset = validated_proposal.output().asset()
assert input_amount == utxo_size
assert output_amount == issued_asset_units
assert input_asset == policy_asset
assert output_asset == asset

# ANCHOR: liquidex_take
# (taker) Accept the proposal
builder2 = network.tx_builder()
builder2.liquidex_take([validated_proposal])
pset2 = builder2.finish(taker)
assert pset2.inputs()[1].sighash() == 1 # ANCHOR: ignore
signed_pset2 = signer2.sign(pset2)

# (taker) Finalize and broadcast the transaction
finalized_pset = taker.finalize(signed_pset2)
tx = finalized_pset.extract_tx()
txid = client.broadcast(tx)
# ANCHOR_END: liquidex_take
maker.wait_for_tx(txid, client)
taker.wait_for_tx(txid, client)

# Verify the final balance
assert(maker.balance()[asset] == issued_asset_units)   
assert asset not in taker.balance()


# (maker) A single key wallet can make a proposal too, signing with its secret key
maker_secret_key = SecretKey.from_bytes(bytes.fromhex("aa" * 32))
maker_public_key = "026a04ab98d9e4774ad806e302dddeb63bea16b5cb5f223ee77478e861bb583eb3"  # of "aa" * 32
view_key = "11" * 32
maker_single_key = Wollet(network, WolletDescriptor(f"ct({view_key},elwpkh({maker_public_key}))"), datadir=None)
txid = node.send_to_address(maker_single_key.address(None).address(), utxo_size, asset=None)
maker_single_key.wait_for_tx(txid, client)

builder = network.tx_builder()
utxo = maker_single_key.utxos()[0].outpoint()
builder.liquidex_make(utxo, maker_single_key.address(None).address(), issued_asset_units, asset)
pset = builder.finish(maker_single_key)

options = SignOptions.allow_sighash_single_anyonecanpay()
signed_pset = maker_secret_key.sign_with_options(pset, options)

proposal = UnvalidatedLiquidexProposal.from_pset(signed_pset)
validated_proposal = proposal.validate(client.get_tx(proposal.needed_tx()))
assert validated_proposal.input().amount() == utxo_size
