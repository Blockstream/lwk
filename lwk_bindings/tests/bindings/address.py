from lwk import *

view_key = "3e129856c574c66d94023ac98b7f69aca9774d10aee4dc087f0c52a498687189"
xpub = "tpubDC8msFGeGuwnKG9Upg7DM2b4DaRqg3CUZa5g8v2SRQ6K4NSkxUgd7HsL2XVWbVm39yBA4LAxysQAm397zwQSQoQgewGiYZqrA9DsP4zbQ1M"
network = Network.testnet()

desc = WolletDescriptor(f"ct({view_key},elwpkh([73c5da0a/84h/1h/0h]{xpub}/<0;1>/*))")
wollet = Wollet(network, desc, datadir=None)

# The full derivation path of an address, e.g. to sign a message with its key
assert str(wollet.address(3).derivation_path()) == "84'/1'/0'/0/3"
assert str(wollet.change(3).derivation_path()) == "84'/1'/0'/1/3"

# Without key origin the path from the master key is unknown
desc = WolletDescriptor(f"ct({view_key},elwpkh({xpub}/<0;1>/*))")
wollet = Wollet(network, desc, datadir=None)
assert wollet.address(3).derivation_path() is None
