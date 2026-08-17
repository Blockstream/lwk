from lwk import *

b = ElementsParamsBuilder()
b.with_policy_asset("aa" * 32)
b.with_genesis_hash("00" * 32)
b.with_parent_genesis_hash("11" * 32)
n = b.build_network()
assert n.policy_asset() == "aa" * 32
assert n.genesis_block_hash() == "00" * 32
assert n.parent_genesis_hash() == "11" * 32
