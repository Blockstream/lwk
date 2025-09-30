from lwk import *

mnemonic = Mnemonic("abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about")
network = Network.testnet()
client = network.default_electrum_client()

lightning_session = LightningSession(network, client)

