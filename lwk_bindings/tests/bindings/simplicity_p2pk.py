import os
from lwk import *

_SIMF_DIR = os.path.join(os.path.dirname(
    __file__), "..", "..", "..", "lwk_simplicity", "data")
P2PK_SOURCE = open(os.path.join(_SIMF_DIR, "p2pk.simf")).read()

TEST_PRIVATE_KEY = "0000000000000000000000000000000000000000000000000000000000000001"
TEST_X_ONLY_PUBLIC_KEY = "79be667ef9dcbbac55a06295ce870b07029bfcdb2dce28d959f2815b16f81798"
TEST_UNSIGNED_TX = "02000000000113226c2af4a18516258790b9c6f118afdf0bfe9cb0cf79574807ddf6bf680be80000000000ffffffff0301499a818545f6bae39fc03b637f2a4e1e64e590cac1bc3a6f6d71aa4443654c140100000000000003e800225120f8b916e58321fccfb61529245bcae76bdf0cedbcb0df2b23c2ac8f1adfed577501499a818545f6bae39fc03b637f2a4e1e64e590cac1bc3a6f6d71aa4443654c140100000000000181be00225120f8b916e58321fccfb61529245bcae76bdf0cedbcb0df2b23c2ac8f1adfed577501499a818545f6bae39fc03b637f2a4e1e64e590cac1bc3a6f6d71aa4443654c140100000000000000fa000000000000"
TEST_UTXO_VALUE = 100000

# These are computed from the private key above; values are printed on first run
TEST_CMR = "1abc423d746ee0fff5bb8ad259bec3b89d319f08a425d6b8092e9c1c5ec7d465"
TEST_ADDRESS = "tex1peavhc0s5wcm0ans49jxg445enyh6uuwl8radea7expf2syt5rkzqjre6vm"
TEST_UTXO_SCRIPT_PUBKEY = "5120cf597c3e147636fece152c8c8ad699992fae71df38fadcf7d93052a811741d84"

network = Network.testnet()
genesis_hash = network.genesis_block_hash()

assert genesis_hash == "a771da8e52ee6ad581ed1e9a99825e5b3b7992225534eaa2ae23244fe26ab1c1"
assert len(genesis_hash) == 64

# Build keypair and verify public key matches expected
keypair = Keypair.from_secret_bytes(bytes.fromhex(TEST_PRIVATE_KEY))
xonly = keypair.x_only_public_key()
assert str(xonly) == TEST_X_ONLY_PUBLIC_KEY

# Test loading p2pk program with public key argument
args = SimplicityArguments()
args = args.add_value(
    "PUBLIC_KEY", SimplicityTypedValue.u256(xonly.to_bytes()))

program = SimplicityProgram.load(P2PK_SOURCE, args)
cmr_from_program = program.cmr()
print("TEST_CMR =", repr(str(cmr_from_program)))
assert str(cmr_from_program) == TEST_CMR

cmr_from_str = Cmr.from_string(TEST_CMR)
cmr_bytes = cmr_from_str.to_bytes()
cmr_from_bytes = Cmr.from_bytes(cmr_bytes)

assert str(cmr_from_str) == TEST_CMR
assert cmr_from_bytes.to_bytes() == cmr_bytes
assert str(cmr_from_bytes) == TEST_CMR
assert cmr_from_program.to_bytes() == cmr_bytes

# Test creating P2TR address for p2pk program (uses NUMS internal key)
address = program.create_p2tr_address(network)
assert address is not None
print("TEST_ADDRESS =", repr(str(address)))
print("TEST_UTXO_SCRIPT_PUBKEY =", repr(str(address.script_pubkey())))
assert str(address) == TEST_ADDRESS
assert str(address.script_pubkey()) == TEST_UTXO_SCRIPT_PUBKEY

# Test creating TxOut from explicit values
utxo = TxOut.from_explicit(
    address.script_pubkey(), network.policy_asset(), TEST_UTXO_VALUE)
assert utxo is not None
assert utxo.value() == TEST_UTXO_VALUE

# Test full transaction finalization
tx = Transaction.from_string(TEST_UNSIGNED_TX)

sighash = program.get_sighash_all(tx, [utxo], 0, network)
sig_hex = keypair.sign_schnorr(sighash.hex())
print("TEST_SIGNATURE =", repr(sig_hex))

witness = SimplicityWitnessValues()
witness = witness.add_value(
    "SIGNATURE", SimplicityTypedValue.byte_array(bytes.fromhex(sig_hex)))
assert witness is not None

finalized_tx = program.finalize_transaction(
    tx, [utxo], 0,
    witness, network, SimplicityLogLevel.NONE
)

assert finalized_tx is not None
print("TEST_FINALIZED_TX =", repr(str(finalized_tx)))

# Test SimplicityType constructors
t_u32 = SimplicityType.u32()
t_u64 = SimplicityType.u64()
t_u256 = SimplicityType.u256()
t_bool = SimplicityType.boolean()
t_either = SimplicityType.either(t_u32, t_bool)
t_option = SimplicityType.option(t_u64)
t_tuple = SimplicityType.tuple([t_u32, t_u256])
t_parsed = SimplicityType.from_string("Either<u32, bool>")
assert str(SimplicityType.from_string(str(t_tuple))) == str(t_tuple)

# Test SimplicityTypedValue constructors
v_u32 = SimplicityTypedValue.u32(42)
v_u64 = SimplicityTypedValue.u64(1000)
v_bool = SimplicityTypedValue.boolean(True)
v_u256 = SimplicityTypedValue.u256(bytes.fromhex(TEST_X_ONLY_PUBLIC_KEY))
v_left = SimplicityTypedValue.left(v_u32, t_bool)
v_right = SimplicityTypedValue.right(t_u32, v_bool)
v_tuple = SimplicityTypedValue.tuple([v_u32, v_u256])
v_none = SimplicityTypedValue.none(t_u64)
v_some = SimplicityTypedValue.some(v_u64)
v_parsed = SimplicityTypedValue.parse("Left(42)", t_either)

# Test add_value on builders
args2 = SimplicityArguments()
args2 = args2.add_value("MY_PARAM", v_u32)
witness2 = SimplicityWitnessValues()
witness2 = witness2.add_value("MY_WITNESS", v_left)

# Verify add_value works for loading a program (regression)
args3 = SimplicityArguments()
args3 = args3.add_value("PUBLIC_KEY", SimplicityTypedValue.u256(
    bytes.fromhex(TEST_X_ONLY_PUBLIC_KEY)))
program2 = SimplicityProgram.load(P2PK_SOURCE, args3)
assert str(program2.cmr()) == TEST_CMR

TEST_CONTRACT_HASH_HEX = "0000460186f7233c927e7db2dcc703c0e500b653ca82273b7bfad8045d85a470"

contract_hash_str = ContractHash.from_string(TEST_CONTRACT_HASH_HEX)

contract_hash_bytes = contract_hash_str.to_bytes()
contract_hash_from_bytes = ContractHash.from_bytes(contract_hash_bytes)

assert str(contract_hash_str) == TEST_CONTRACT_HASH_HEX

assert contract_hash_from_bytes.to_bytes() == contract_hash_bytes
assert str(contract_hash_from_bytes) == TEST_CONTRACT_HASH_HEX

TEST_PUBLIC_KEY = "0279be667ef9dcbbac55a06295ce870b07029bfcdb2dce28d959f2815b16f81798"

public_key_str = PublicKey.from_string(TEST_PUBLIC_KEY)

public_key_bytes = public_key_str.to_bytes()
public_key_from_bytes = PublicKey.from_bytes(public_key_bytes)

assert str(public_key_str) == TEST_PUBLIC_KEY

assert public_key_from_bytes.to_bytes() == public_key_bytes
assert str(public_key_from_bytes) == TEST_PUBLIC_KEY

x_only_public_key_str = XOnlyPublicKey.from_string(TEST_X_ONLY_PUBLIC_KEY)

x_only_public_key_bytes = x_only_public_key_str.to_bytes()
x_only_public_key_from_bytes = XOnlyPublicKey.from_bytes(
    x_only_public_key_bytes)

assert str(x_only_public_key_str) == TEST_X_ONLY_PUBLIC_KEY

assert x_only_public_key_from_bytes.to_bytes() == x_only_public_key_bytes
assert str(x_only_public_key_from_bytes) == TEST_X_ONLY_PUBLIC_KEY

TEST_TWEAK_KEY = "0000460186f7233c927e7db2dcc703c0e500b653ca82273b7bfad8045d85a470"

tweak_str = Tweak.from_string(TEST_TWEAK_KEY)

tweak_bytes = tweak_str.to_bytes()
tweak_from_bytes = Tweak.from_bytes(tweak_bytes)

assert str(tweak_str) == TEST_TWEAK_KEY

assert tweak_from_bytes.to_bytes() == tweak_bytes
assert str(tweak_from_bytes) == TEST_TWEAK_KEY
