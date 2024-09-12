from lwk import *

mnemonic = Mnemonic("abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about")
network = Network.testnet()
assert(str(network) == "LiquidTestnet")

client = network.default_electrum_client()
client.ping()

signer = Signer(mnemonic, network)
desc = signer.wpkh_slip77_descriptor()

assert(str(desc) == "ct(slip77(9c8e4f05c7711a98c838be228bcb84924d4570ca53f35fa1c793e58841d47023),elwpkh([73c5da0a/84'/1'/0']tpubDC8msFGeGuwnKG9Upg7DM2b4DaRqg3CUZa5g8v2SRQ6K4NSkxUgd7HsL2XVWbVm39yBA4LAxysQAm397zwQSQoQgewGiYZqrA9DsP4zbQ1M/<0;1>/*))#2e4n992d")

wollet = Wollet(network, desc, datadir=None)
update = client.full_scan(wollet)
wollet.apply_update(update)

assert(len(wollet.transactions()) >= 99)
