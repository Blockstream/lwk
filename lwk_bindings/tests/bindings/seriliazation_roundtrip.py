from lwk import *

TEST_SCRIPT_PUBKEY = "5120f8b916e58321fccfb61529245bcae76bdf0cedbcb0df2b23c2ac8f1adfed5775"

script_pubkey = Script.from_string(TEST_SCRIPT_PUBKEY)
assert str(script_pubkey) == TEST_SCRIPT_PUBKEY
assert str(Script.from_bytes(script_pubkey.to_bytes())) == TEST_SCRIPT_PUBKEY

TEST_TXID = "ae2240a6402cbce14546a56a22b35b7f2758c479cd9be836df2a9926ed926981"

txid = Txid.from_string(TEST_TXID)
assert str(txid) == TEST_TXID
assert str(Txid.from_bytes(txid.to_bytes())) == TEST_TXID

