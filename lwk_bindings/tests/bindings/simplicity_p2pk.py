from lwk import *

P2PK_SOURCE = """
fn main() {
    jet::bip_0340_verify((param::PUBLIC_KEY, jet::sig_all_hash()), witness::SIGNATURE)
}
"""

TEST_PUBLIC_KEY = "8a65c55726dc32b59b649ad0187eb44490de681bb02601b8d3f58c8b9fff9083"
TEST_UNSIGNED_TX = "02000000000113226c2af4a18516258790b9c6f118afdf0bfe9cb0cf79574807ddf6bf680be80000000000ffffffff0301499a818545f6bae39fc03b637f2a4e1e64e590cac1bc3a6f6d71aa4443654c140100000000000003e800225120f8b916e58321fccfb61529245bcae76bdf0cedbcb0df2b23c2ac8f1adfed577501499a818545f6bae39fc03b637f2a4e1e64e590cac1bc3a6f6d71aa4443654c140100000000000181be00225120f8b916e58321fccfb61529245bcae76bdf0cedbcb0df2b23c2ac8f1adfed577501499a818545f6bae39fc03b637f2a4e1e64e590cac1bc3a6f6d71aa4443654c140100000000000000fa000000000000"
TEST_UTXO_SCRIPT_PUBKEY = "5120f8b916e58321fccfb61529245bcae76bdf0cedbcb0df2b23c2ac8f1adfed5775"
TEST_UTXO_VALUE = 100000
TEST_SIGNATURE = "ab5173c154e62da5e4f5d983177d7398918d61d6149f3b1bb7271d00d165391c19f86c53c5d6dcfcd50f4fc4dbf8748fbb6a1614ddacd663818bfc90ef8f818d"
TEST_FINALIZED_TX = "02000000010113226c2af4a18516258790b9c6f118afdf0bfe9cb0cf79574807ddf6bf680be80000000000ffffffff0301499a818545f6bae39fc03b637f2a4e1e64e590cac1bc3a6f6d71aa4443654c140100000000000003e800225120f8b916e58321fccfb61529245bcae76bdf0cedbcb0df2b23c2ac8f1adfed577501499a818545f6bae39fc03b637f2a4e1e64e590cac1bc3a6f6d71aa4443654c140100000000000181be00225120f8b916e58321fccfb61529245bcae76bdf0cedbcb0df2b23c2ac8f1adfed577501499a818545f6bae39fc03b637f2a4e1e64e590cac1bc3a6f6d71aa4443654c140100000000000000fa00000000000000000440ab5173c154e62da5e4f5d983177d7398918d61d6149f3b1bb7271d00d165391c19f86c53c5d6dcfcd50f4fc4dbf8748fbb6a1614ddacd663818bfc90ef8f818d76e06922d314cb8aae4db8656b36c935a030fd688921bcd037604c0371a7eb19173fff21060a15c38735d772f07a13a9e8c81c672ff1b725794653e7b2a65b5569ecb5da314478a8c43860188599c502d8d8c399c5510915d0998008ab858317852c27b030a85d612fd7152321658700adbbf004300c4020b685a4424842507d7d747e6611a740d8c421038e9744e75d423d0e2e9f164d0221bf8a65c55726dc32b59b649ad0187eb44490de681bb02601b8d3f58c8b9fff908300000000000000"

# Expected CMR for P2PK program with TEST_PUBLIC_KEY
TEST_CMR = "b685a4424842507d7d747e6611a740d8c421038e9744e75d423d0e2e9f164d02"
TEST_ADDRESS = "tex1plzu3devry87vlds49yj9hjh8d00semdukr0jkg7z4j834hld2a6s6y4amk"

def test_p2pk_load_program():
    """Test loading p2pk program with public key argument"""
    args = SimplicityArguments()
    args = args.add_bytes("PUBLIC_KEY", TEST_PUBLIC_KEY)

    program = simplicity_load_program(P2PK_SOURCE, args)

    cmr = program.cmr()
    assert cmr == TEST_CMR


def test_p2pk_create_address():
    """Test creating P2TR address for p2pk program"""
    args = SimplicityArguments()
    args = args.add_bytes("PUBLIC_KEY", TEST_PUBLIC_KEY)

    program = simplicity_load_program(P2PK_SOURCE, args)
    network = Network.testnet()

    address = simplicity_create_p2tr_address(program, TEST_PUBLIC_KEY, network)

    assert address is not None
    assert str(address) == TEST_ADDRESS

def test_p2pk_finalize_transaction():
    """Test full transaction finalization with real test vectors"""
    # Build program
    args = SimplicityArguments()
    args = args.add_bytes("PUBLIC_KEY", TEST_PUBLIC_KEY)
    program = simplicity_load_program(P2PK_SOURCE, args)

    network = Network.testnet()

    # Create transaction and UTXO
    tx = Transaction(TEST_UNSIGNED_TX)
    utxo = TxOut.from_explicit(TEST_UTXO_SCRIPT_PUBKEY, network.policy_asset(), TEST_UTXO_VALUE)

    genesis_hash = network.genesis_block_hash()

    # Build witness with the known signature
    witness = SimplicityWitnessValues()
    witness = witness.add_bytes("SIGNATURE", TEST_SIGNATURE)

    # Finalize the transaction
    finalized_tx = simplicity_finalize_transaction(
        tx, program, TEST_PUBLIC_KEY, [utxo], 0,
        witness, network, genesis_hash, SimplicityLogLevel.NONE
    )

    assert finalized_tx is not None
    finalized_hex = str(finalized_tx)
    assert finalized_hex == TEST_FINALIZED_TX


def test_p2pk_witness_values():
    """Test building witness values with signature (64 bytes)"""
    witness = SimplicityWitnessValues()
    witness = witness.add_bytes("SIGNATURE", TEST_SIGNATURE)

    assert witness is not None


def test_p2pk_with_number():
    """Test adding numeric values (for other programs that need them)"""
    args = SimplicityArguments()
    args = args.add_number("THRESHOLD", 2)

    assert args is not None


def test_txout_from_explicit():
    """Test creating TxOut from explicit values"""
    network = Network.testnet()

    utxo = TxOut.from_explicit(
        TEST_UTXO_SCRIPT_PUBKEY,
        network.policy_asset(),
        TEST_UTXO_VALUE
    )

    assert utxo is not None
    assert utxo.value() == TEST_UTXO_VALUE

def test_network_genesis_hash():
    """Test getting genesis block hash from network"""
    network = Network.testnet()
    genesis_hash = network.genesis_block_hash()

    assert genesis_hash == "a771da8e52ee6ad581ed1e9a99825e5b3b7992225534eaa2ae23244fe26ab1c1"
    assert len(genesis_hash) == 64

if __name__ == "__main__":
    test_network_genesis_hash()
    test_p2pk_load_program()
    test_p2pk_create_address()
    test_p2pk_witness_values()
    test_p2pk_with_number()
    test_txout_from_explicit()
    test_p2pk_finalize_transaction()
    print("All p2pk tests passed!")
