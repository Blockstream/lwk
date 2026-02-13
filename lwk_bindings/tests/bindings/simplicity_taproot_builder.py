from lwk import *

state = bytearray(32)
state[31] = 1

cmr = Cmr.from_hex(
    "cbd8d3d0cc95384237c1bf20334c30b579f22058563c37731a3ab2bc76d5a248"
)
internal_key = XOnlyPublicKey(
    "50929b74c1a04954b78b4b6035e97a5e078a5a0f28ec96d547bfee9ace803ac0"
)

spend_info = (
    StateTaprootBuilder()
    .add_simplicity_leaf(1, cmr)
    .add_data_leaf(1, bytes(state))
    .finalize(internal_key)
)

assert str(spend_info.script_pubkey()) == "51205920ca2ef73fa8c0378b50e99e4518b72fee1c413c1f1c52acbde479b3ec0a21"
