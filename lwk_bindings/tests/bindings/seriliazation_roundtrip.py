from lwk import *

def assert_roundtrip(obj_class, hex_string):
    obj = obj_class.from_string(hex_string)
    assert str(obj) == hex_string
    assert str(obj_class.from_bytes(obj.to_bytes())) == hex_string

TEST_SCRIPT_PUBKEY = "5120f8b916e58321fccfb61529245bcae76bdf0cedbcb0df2b23c2ac8f1adfed5775"
TEST_TXID = "ae2240a6402cbce14546a56a22b35b7f2758c479cd9be836df2a9926ed926981"

assert_roundtrip(Script, TEST_SCRIPT_PUBKEY)
assert_roundtrip(Txid, TEST_TXID)
