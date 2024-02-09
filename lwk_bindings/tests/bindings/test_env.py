from lwk_bindings import *

t = TestEnv() # launch electrs and elementsd

assert(t.height() == 101)

t.generate(10)

assert(t.height() == 111)

node_address = t.getnewaddress()

t.sendtoaddress(node_address, 10000, None)