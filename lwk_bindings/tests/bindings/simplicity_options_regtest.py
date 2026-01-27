import os
import time
from lwk import *

_TEST_DATA = os.path.join(os.path.dirname(__file__), "..", "test_data")
P2PK_SOURCE = open(os.path.join(_TEST_DATA, "p2pk.simf")).read()
OPTIONS_SOURCE = open(os.path.join(_TEST_DATA, "options.simf")).read()


def build_options_arguments(params):
    args = SimplicityArguments()
    for key in ["START_TIME", "EXPIRY_TIME"]:
        args = args.add_value(key, SimplicityTypedValue.u32(params[key]))

    for key in ["COLLATERAL_PER_CONTRACT", "SETTLEMENT_PER_CONTRACT"]:
        args = args.add_value(key, SimplicityTypedValue.u64(params[key]))

    for key in [
        "COLLATERAL_ASSET_ID", "SETTLEMENT_ASSET_ID",
        "ISSUANCE_ASSET_ENTROPY",
        "OPTION_OUTPOINT_TXID", "GRANTOR_OUTPOINT_TXID",
        "OPTION_TOKEN_ASSET", "OPTION_REISSUANCE_TOKEN_ASSET",
        "GRANTOR_TOKEN_ASSET", "GRANTOR_REISSUANCE_TOKEN_ASSET",
    ]:
        args = args.add_value(key, SimplicityTypedValue.u256(params[key]))

    for key in ["OPTION_OUTPOINT_VOUT", "GRANTOR_OUTPOINT_VOUT"]:
        args = args.add_value(key, SimplicityTypedValue.u32(params[key]))

    for key in ["OPTION_CONFIDENTIAL", "GRANTOR_CONFIDENTIAL"]:
        args = args.add_value(key, SimplicityTypedValue.boolean(params[key]))

    return args


def wait_for_tx(client, txid, retries=30):
    for _ in range(retries):
        try:
            return client.get_tx(txid)
        except LwkError:
            time.sleep(1)
    raise RuntimeError(f"Could not fetch tx {txid} after {retries} retries")


def find_output_by_script(tx, script_hex):
    for i, output in enumerate(tx.outputs()):
        if str(output.script_pubkey()) == script_hex:
            return i, output
    raise RuntimeError("Output not found for script")


def find_outputs_by_script(tx, script_hex):
    results = []
    for i, output in enumerate(tx.outputs()):
        if str(output.script_pubkey()) == script_hex:
            results.append((i, output))
    return results



COLLATERAL_PER_CONTRACT = 1000
SETTLEMENT_PER_CONTRACT = 500
NUM_CONTRACTS = 10
START_TIME = 1
EXPIRY_TIME = 2

# Step 1: Setup
node = LwkTestEnv()
network = Network.regtest_default()
policy_asset = network.policy_asset()
genesis_hash = node.genesis_block_hash()
client = ElectrumClient.from_url(node.electrum_url())

mnemonic = Mnemonic("abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about")
signer = Signer(mnemonic, network)
derivation_path = "m/86'/1'/0'/0/0"
xonly_pubkey = simplicity_derive_xonly_pubkey(signer, derivation_path)

p2pk_args = SimplicityArguments()
p2pk_args = p2pk_args.add_value("PUBLIC_KEY", SimplicityTypedValue.u256(xonly_pubkey.to_hex()))
p2pk_program = SimplicityProgram.load(P2PK_SOURCE, p2pk_args)
p2pk_address = p2pk_program.create_p2tr_address(xonly_pubkey, network)
p2pk_script = p2pk_address.script_pubkey()
p2pk_script_hex = str(p2pk_script)

blinder_keypair = Keypair.from_secret_bytes(bytes([1] * 32))
blinding_pubkey = blinder_keypair.public_key()

# Step 2: Fund P2PK address
funded_sats = 100000

txid1 = node.send_to_address(p2pk_address, funded_sats, asset=None)
node.generate(1)
txid2 = node.send_to_address(p2pk_address, funded_sats, asset=None)
node.generate(1)
txid3 = node.send_to_address(p2pk_address, funded_sats, asset=None)
node.generate(1)

funding_tx1 = wait_for_tx(client, txid1)
funding_tx2 = wait_for_tx(client, txid2)
funding_tx3 = wait_for_tx(client, txid3)

vout1, output1 = find_output_by_script(funding_tx1, p2pk_script_hex)
vout2, output2 = find_output_by_script(funding_tx2, p2pk_script_hex)
vout3, output3 = find_output_by_script(funding_tx3, p2pk_script_hex)

# Step 3: Build creation transaction

option_contract_hash = ContractHash.from_hex("0000000000000000000000000000000000000000000000000000000000000001")
grantor_contract_hash = ContractHash.from_hex("0000000000000000000000000000000000000000000000000000000000000002")

outpoint0 = OutPoint.from_parts(txid1, vout1)
inp_builder0 = PsetInputBuilder.from_prevout(outpoint0)
inp_builder0.witness_utxo(output1)
inp_builder0.sequence(TxSequence.zero())
inp_builder0.issuance_inflation_keys(1)
inp_builder0.issuance_asset_entropy(bytes(option_contract_hash.to_bytes()).hex())
inp_builder0.blinded_issuance(0x00)
creation_input0 = inp_builder0.build()

outpoint1 = OutPoint.from_parts(txid2, vout2)
inp_builder1 = PsetInputBuilder.from_prevout(outpoint1)
inp_builder1.witness_utxo(output2)
inp_builder1.sequence(TxSequence.zero())
inp_builder1.issuance_inflation_keys(1)
inp_builder1.issuance_asset_entropy(bytes(grantor_contract_hash.to_bytes()).hex())
inp_builder1.blinded_issuance(0x00)
creation_input1 = inp_builder1.build()

option_token_asset = asset_id_from_issuance(outpoint0, option_contract_hash)
option_reissuance_token_asset = reissuance_token_from_issuance(outpoint0, option_contract_hash, False)

grantor_token_asset = asset_id_from_issuance(outpoint1, grantor_contract_hash)
grantor_reissuance_token_asset = reissuance_token_from_issuance(outpoint1, grantor_contract_hash, False)

option_issuance_ids = creation_input0.issuance_ids()
assert option_issuance_ids is not None
assert str(option_issuance_ids[0]) == option_token_asset
assert str(option_issuance_ids[1]) == option_reissuance_token_asset

grantor_issuance_ids = creation_input1.issuance_ids()
assert grantor_issuance_ids is not None
assert str(grantor_issuance_ids[0]) == grantor_token_asset
assert str(grantor_issuance_ids[1]) == grantor_reissuance_token_asset

issuance_entropy = generate_asset_entropy(outpoint0, option_contract_hash)

options_params = {
    "START_TIME": START_TIME,
    "EXPIRY_TIME": EXPIRY_TIME,
    "COLLATERAL_PER_CONTRACT": COLLATERAL_PER_CONTRACT,
    "SETTLEMENT_PER_CONTRACT": SETTLEMENT_PER_CONTRACT,
    "COLLATERAL_ASSET_ID": asset_id_inner_hex(policy_asset),
    "SETTLEMENT_ASSET_ID": asset_id_inner_hex(policy_asset),
    "ISSUANCE_ASSET_ENTROPY": issuance_entropy,
    "OPTION_OUTPOINT_TXID": bytes(txid1.bytes()).hex(),
    "OPTION_OUTPOINT_VOUT": vout1,
    "OPTION_CONFIDENTIAL": True,
    "GRANTOR_OUTPOINT_TXID": bytes(txid2.bytes()).hex(),
    "GRANTOR_OUTPOINT_VOUT": vout2,
    "GRANTOR_CONFIDENTIAL": True,
    "OPTION_TOKEN_ASSET": asset_id_inner_hex(option_token_asset),
    "OPTION_REISSUANCE_TOKEN_ASSET": asset_id_inner_hex(option_reissuance_token_asset),
    "GRANTOR_TOKEN_ASSET": asset_id_inner_hex(grantor_token_asset),
    "GRANTOR_REISSUANCE_TOKEN_ASSET": asset_id_inner_hex(grantor_reissuance_token_asset),
}
options_args = build_options_arguments(options_params)
options_program = SimplicityProgram.load(OPTIONS_SOURCE, options_args)

contract_address = options_program.create_p2tr_address(xonly_pubkey, network)
contract_script = contract_address.script_pubkey()
contract_script_hex = str(contract_script)

fee_sats = 500

out_builder0 = PsetOutputBuilder.new_explicit(
    contract_script, 1, option_reissuance_token_asset, blinding_pubkey
)
out_builder0.blinder_index(0)
creation_output0 = out_builder0.build()

out_builder1 = PsetOutputBuilder.new_explicit(
    contract_script, 1, grantor_reissuance_token_asset, blinding_pubkey
)
out_builder1.blinder_index(1)
creation_output1 = out_builder1.build()

total_input_sats = funded_sats * 2
change_sats = total_input_sats - fee_sats
out_builder2 = PsetOutputBuilder.new_explicit(
    p2pk_script, change_sats, policy_asset, None
)
creation_output2 = out_builder2.build()

fee_script = Script.empty()
out_builder3 = PsetOutputBuilder.new_explicit(
    fee_script, fee_sats, policy_asset, None
)
creation_output3 = out_builder3.build()

pset_builder = PsetBuilder.new_v2()
pset_builder.add_input(creation_input0)
pset_builder.add_input(creation_input1)
pset_builder.add_output(creation_output0)
pset_builder.add_output(creation_output1)
pset_builder.add_output(creation_output2)
pset_builder.add_output(creation_output3)

inp_secrets = {
    0: TxOutSecrets.from_explicit(policy_asset, funded_sats),
    1: TxOutSecrets.from_explicit(policy_asset, funded_sats),
}
pset_builder.blind_last(inp_secrets)

creation_pset = pset_builder.build()
creation_tx = creation_pset.extract_tx()

try:
    creation_tx.verify_tx_amt_proofs([output1, output2])
except Exception as e:
    print(f"  Creation tx balance check FAILED: {e}")

# Step 4: Sign and broadcast creation tx

creation_utxos = [output1, output2]

sig0 = p2pk_program.create_p2pk_signature(
    signer, derivation_path, creation_tx,
    creation_utxos, 0, network, genesis_hash
)
witness0 = SimplicityWitnessValues()
witness0 = witness0.add_value("SIGNATURE", SimplicityTypedValue.byte_array(str(sig0)))
creation_tx = p2pk_program.finalize_transaction(
    creation_tx, xonly_pubkey, creation_utxos, 0,
    witness0, network, genesis_hash, SimplicityLogLevel.NONE
)

sig1 = p2pk_program.create_p2pk_signature(
    signer, derivation_path, creation_tx,
    creation_utxos, 1, network, genesis_hash
)
witness1 = SimplicityWitnessValues()
witness1 = witness1.add_value("SIGNATURE", SimplicityTypedValue.byte_array(str(sig1)))
creation_tx = p2pk_program.finalize_transaction(
    creation_tx, xonly_pubkey, creation_utxos, 1,
    witness1, network, genesis_hash, SimplicityLogLevel.NONE
)

creation_txid = client.broadcast(creation_tx)
node.generate(1)

# Step 5: Fetch creation outputs

confirmed_creation_tx = wait_for_tx(client, creation_txid)

contract_outputs = find_outputs_by_script(confirmed_creation_tx, contract_script_hex)
assert len(contract_outputs) >= 2, f"Expected at least 2 contract outputs, got {len(contract_outputs)}"

creation_out_vout0 = contract_outputs[0][0]
creation_out_output0 = contract_outputs[0][1]
creation_out_vout1 = contract_outputs[1][0]
creation_out_output1 = contract_outputs[1][1]

blinding_sk = blinder_keypair.secret_key()

secrets0 = creation_out_output0.unblind(blinding_sk)
secrets1 = creation_out_output1.unblind(blinding_sk)

assert secrets0.value() == 1
assert secrets1.value() == 1

# Step 6: Build funding transaction

option_token_entropy = generate_asset_entropy(outpoint0, option_contract_hash)
grantor_token_entropy = generate_asset_entropy(outpoint1, grantor_contract_hash)

collateral_sats = COLLATERAL_PER_CONTRACT * NUM_CONTRACTS
settlement_sats = SETTLEMENT_PER_CONTRACT * NUM_CONTRACTS

funding_outpoint0 = OutPoint.from_parts(creation_txid, creation_out_vout0)
fund_inp_builder0 = PsetInputBuilder.from_prevout(funding_outpoint0)
fund_inp_builder0.witness_utxo(creation_out_output0)
fund_inp_builder0.sequence(TxSequence.zero())
fund_inp_builder0.issuance_value_amount(NUM_CONTRACTS)
fund_inp_builder0.issuance_asset_entropy(option_token_entropy)
fund_inp_builder0.blinded_issuance(0x00)
fund_inp_builder0.issuance_blinding_nonce(Tweak.from_hex(secrets0.asset_blinding_factor().to_hex()))
funding_input0 = fund_inp_builder0.build()

funding_outpoint1 = OutPoint.from_parts(creation_txid, creation_out_vout1)
fund_inp_builder1 = PsetInputBuilder.from_prevout(funding_outpoint1)
fund_inp_builder1.witness_utxo(creation_out_output1)
fund_inp_builder1.sequence(TxSequence.zero())
fund_inp_builder1.issuance_value_amount(NUM_CONTRACTS)
fund_inp_builder1.issuance_asset_entropy(grantor_token_entropy)
fund_inp_builder1.blinded_issuance(0x00)
fund_inp_builder1.issuance_blinding_nonce(Tweak.from_hex(secrets1.asset_blinding_factor().to_hex()))
funding_input1 = fund_inp_builder1.build()

outpoint2 = OutPoint.from_parts(txid3, vout3)
fund_inp_builder2 = PsetInputBuilder.from_prevout(outpoint2)
fund_inp_builder2.witness_utxo(output3)
fund_inp_builder2.sequence(TxSequence.zero())
funding_input2 = fund_inp_builder2.build()

funding_fee_sats = 500

fund_out_builder0 = PsetOutputBuilder.new_explicit(
    contract_script, 1, option_reissuance_token_asset, blinding_pubkey
)
fund_out_builder0.blinder_index(0)
funding_output0 = fund_out_builder0.build()

fund_out_builder1 = PsetOutputBuilder.new_explicit(
    contract_script, 1, grantor_reissuance_token_asset, blinding_pubkey
)
fund_out_builder1.blinder_index(1)
funding_output1 = fund_out_builder1.build()

fund_out_builder2 = PsetOutputBuilder.new_explicit(
    contract_script, collateral_sats, policy_asset, None
)
funding_output2 = fund_out_builder2.build()

fund_out_builder3 = PsetOutputBuilder.new_explicit(
    p2pk_script, NUM_CONTRACTS, option_token_asset, None
)
funding_output3 = fund_out_builder3.build()

fund_out_builder4 = PsetOutputBuilder.new_explicit(
    p2pk_script, NUM_CONTRACTS, grantor_token_asset, None
)
funding_output4 = fund_out_builder4.build()

change_sats_funding = funded_sats - collateral_sats - funding_fee_sats
fund_out_builder5 = PsetOutputBuilder.new_explicit(
    p2pk_script, change_sats_funding, policy_asset, None
)
funding_output5 = fund_out_builder5.build()

fund_out_builder6 = PsetOutputBuilder.new_explicit(
    fee_script, funding_fee_sats, policy_asset, None
)
funding_output6 = fund_out_builder6.build()

fund_pset_builder = PsetBuilder.new_v2()
fund_pset_builder.add_input(funding_input0)
fund_pset_builder.add_input(funding_input1)
fund_pset_builder.add_input(funding_input2)
fund_pset_builder.add_output(funding_output0)
fund_pset_builder.add_output(funding_output1)
fund_pset_builder.add_output(funding_output2)
fund_pset_builder.add_output(funding_output3)
fund_pset_builder.add_output(funding_output4)
fund_pset_builder.add_output(funding_output5)
fund_pset_builder.add_output(funding_output6)

fund_inp_secrets = {
    0: secrets0,
    1: secrets1,
}
fund_pset_builder.blind_last(fund_inp_secrets)

funding_pset = fund_pset_builder.build()
funding_tx = funding_pset.extract_tx()

funding_utxos = [creation_out_output0, creation_out_output1, output3]

# Step 6b: Verify balance immediately after extraction
try:
    funding_tx.verify_tx_amt_proofs(funding_utxos)
except Exception as e:
    print(f"  Balance check FAILED before finalization: {e}")

# Step 7: Sign and broadcast funding tx

funding_outputs_list = funding_tx.outputs()

fund_contract_outputs = []
for i, out in enumerate(funding_outputs_list):
    if str(out.script_pubkey()) == contract_script_hex:
        fund_contract_outputs.append((i, out))

assert len(fund_contract_outputs) >= 2

fund_out_vout0 = fund_contract_outputs[0][0]
fund_out_output0 = fund_contract_outputs[0][1]
fund_out_vout1 = fund_contract_outputs[1][0]
fund_out_output1 = fund_contract_outputs[1][1]

out_secrets0 = fund_out_output0.unblind(blinding_sk)
out_secrets1 = fund_out_output1.unblind(blinding_sk)

path_type = SimplicityType.parse(
    "Either<Either<(u64,u256,u256,u256,u256,u256,u256,u256,u256), Either<(bool,u64,u64,u64),(bool,u64,u64)>>, Either<(bool,u64,u64),(bool,u64,u64)>>"
)

in_opt_abf = secrets0.asset_blinding_factor().to_hex()
in_opt_vbf = secrets0.value_blinding_factor().to_hex()
in_gra_abf = secrets1.asset_blinding_factor().to_hex()
in_gra_vbf = secrets1.value_blinding_factor().to_hex()
out_opt_abf = out_secrets0.asset_blinding_factor().to_hex()
out_opt_vbf = out_secrets0.value_blinding_factor().to_hex()
out_gra_abf = out_secrets1.asset_blinding_factor().to_hex()
out_gra_vbf = out_secrets1.value_blinding_factor().to_hex()

path_value_str = (
    f"Left(Left(({settlement_sats},"
    f"0x{in_opt_abf},"
    f"0x{in_opt_vbf},"
    f"0x{in_gra_abf},"
    f"0x{in_gra_vbf},"
    f"0x{out_opt_abf},"
    f"0x{out_opt_vbf},"
    f"0x{out_gra_abf},"
    f"0x{out_gra_vbf})))"
)
path_value = SimplicityTypedValue.parse(path_value_str, path_type)

funding_witness = SimplicityWitnessValues()
funding_witness = funding_witness.add_value("PATH", path_value)

funding_tx = options_program.finalize_transaction(
    funding_tx, xonly_pubkey, funding_utxos, 0,
    funding_witness, network, genesis_hash, SimplicityLogLevel.NONE
)

funding_tx = options_program.finalize_transaction(
    funding_tx, xonly_pubkey, funding_utxos, 1,
    funding_witness, network, genesis_hash, SimplicityLogLevel.NONE
)

sig2 = p2pk_program.create_p2pk_signature(
    signer, derivation_path, funding_tx,
    funding_utxos, 2, network, genesis_hash
)
witness2 = SimplicityWitnessValues()
witness2 = witness2.add_value("SIGNATURE", SimplicityTypedValue.byte_array(str(sig2)))
funding_tx = p2pk_program.finalize_transaction(
    funding_tx, xonly_pubkey, funding_utxos, 2,
    witness2, network, genesis_hash, SimplicityLogLevel.NONE
)

funding_tx.verify_tx_amt_proofs(funding_utxos)

funding_txid = client.broadcast(funding_tx)
node.generate(1)

# Step 8: Verify

confirmed_funding_tx = wait_for_tx(client, funding_txid)
assert confirmed_funding_tx is not None

fund_inputs = confirmed_funding_tx.inputs()

assert creation_txid is not None
assert funding_txid is not None