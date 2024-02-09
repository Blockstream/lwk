from lwk_bindings import *

t = TestEnv() # launch electrs and elementsd

assert(t.height() == 101)

t.generate(10)

assert(t.height() == 111)

node_address = t.get_new_address()

t.send_to_address(node_address, 10000, None)