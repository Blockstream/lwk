from lwk import *

t = LwkTestEnv() # launch electrs and elementsd

assert(t.height() == 101)

t.generate(10)

assert(t.height() == 111)

node_address = t.get_new_address()

t.send_to_address(node_address, 10000, None)

p = Precision(2)
assert(p.sats_to_string(100) == "1.00")
assert(p.string_to_sats("1") == 100)
