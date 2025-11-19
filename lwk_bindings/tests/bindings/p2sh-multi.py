from lwk import *

# Start nodes
node = LwkTestEnv()

network = Network.regtest_default()
policy_asset = network.policy_asset()
client = ElectrumClient.from_url(node.electrum_url())

# Create receiver wallet
recv_mnemonic = Mnemonic("abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about")

recv_signer = Signer(recv_mnemonic, network)
recv_desc = recv_signer.wpkh_slip77_descriptor()
recv_wollet = Wollet(network, recv_desc, datadir=None)
recv_addr = recv_wollet.address(0).address()
assert recv_wollet.balance().get(policy_asset, 0) == 0

# Create sending wallet
# P2SH 2of3 with 3 single keys blinded in a non standard way
# 3 single keys
wif_aa = "cTJTN1hGHqucsgqmYVbhU3g4eU9g5HzE1sxuSY32M1xap1K4sYHF";
wif_bb = "cTsdXxTC346tsb7HaddDzC5dTqAT8XCsdJsacS4N3ak2mCGGZcN5";
wif_cc = "cUSohuD7nGJAsVNocmekWLVCHCBEBkRXEjnFnL5hk9XUiPBCLR4d";
sk_a = SecretKey.from_wif(wif_aa);
sk_b = SecretKey.from_wif(wif_bb);
sk_c = SecretKey.from_wif(wif_cc);
# from test_non_std_legacy_multisig
pk_a = "026a04ab98d9e4774ad806e302dddeb63bea16b5cb5f223ee77478e861bb583eb3";
pk_b = "0268680737c76dabb801cb2204f57dbe4e4579e4f710cd67dc1b4227592c81e9b5";
pk_c = "02b95c249d84f417e3e395a127425428b540671cc15881eb828c17b722a53fc599";

# A temporary descriptor blinding key
view_key = "1111111111111111111111111111111111111111111111111111111111111111";
# P2SH 2of3 with 3 single pubkeys
desc = f"ct({view_key},elsh(multi(2,{pk_a},{pk_b},{pk_c})))"
desc = WolletDescriptor(desc)
wollet = Wollet(network, desc, datadir=None)

# Get an address with the non-standard blinding public key
blinding_privkey = SecretKey.from_bytes(bytes.fromhex("7777777777777777777777777777777777777777777777777777777777777777"))
# from test_non_std_legacy_multisig
addr_no_std = Address("AzpuC21jFoqV5ueVkYvda8F5EJRb2mEUUnT7vPVhSSUk1AznrKCwnDxwSjtkvuAr5C9nB5HESd9oETe1")
# This has the same script pubkey for wollet.address(None)
# But has blinding pukey corresponding to blinding_privkey 77..77
addr_from_desc = wollet.address(None).address()
assert str(addr_no_std) != str(addr_from_desc)
assert str(addr_no_std.script_pubkey()) == str(addr_from_desc.script_pubkey())

# Send asset to the address with the non-standard blinding key
asset = node.issue_asset(10_000)
txid = node.send_to_address(addr_no_std, 10_000, asset)

# Send BTC to the address from descriptor (for fees)
txid = node.send_to_address(addr_from_desc, 20_000, None)
wollet.wait_for_tx(txid, client)

assert wollet.balance().get(policy_asset, 0) == 20_000
assert wollet.balance().get(str(asset), 0) == 0

# Get external utxo
external_utxos = wollet.unblind_utxos_with(blinding_privkey);

# Create speding tx
builder = network.tx_builder()
# add external utxos
builder.add_external_utxos(external_utxos)
# add recipient
builder.add_recipient(recv_addr, 10_000, asset)
# drain lbtc
builder.drain_lbtc_wallet()
builder.drain_lbtc_to(recv_addr)
pset = builder.finish(wollet)

# sign with seckey
pset = sk_a.sign(pset)
pset = sk_b.sign(pset)

pset = wollet.finalize(pset)
tx = pset.extract_tx()
txid = client.broadcast(tx)
wollet.wait_for_tx(txid, client)
recv_wollet.wait_for_tx(txid, client)

assert wollet.balance().get(policy_asset, 0) == 0
assert wollet.balance().get(str(asset), 0) == 0
assert recv_wollet.balance().get(policy_asset, 0) > 0
assert recv_wollet.balance().get(str(asset), 0) == 10_000
