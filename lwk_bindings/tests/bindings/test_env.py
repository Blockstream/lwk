from lwk_bindings import *

t = TestEnv() # launch electrs and bitcoind

assert(t.height(), 101)

t.generate(10)

assert(t.height(), 111)

