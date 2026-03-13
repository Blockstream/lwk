import tempfile

from lwk import *


mnemonic = Mnemonic("abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about")
network = Network.regtest_default()
signer = Signer(mnemonic, network)
desc = signer.wpkh_slip77_descriptor()

datadir = tempfile.mkdtemp()

builder = WolletBuilder(network, desc)
builder.with_merge_threshold(2)
builder.with_legacy_fs_store(datadir)
wollet = builder.build()

assert wollet is not None
assert wollet.address(0).index() == 0
assert len(wollet.transactions()) == 0
