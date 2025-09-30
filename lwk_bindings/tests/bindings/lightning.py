from lwk import *

mnemonic = Mnemonic("abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about")
network = Network.testnet()

lightning_session = LightningSession(network, "elements-testnet.blockstream.info:50002", True, True)

lightning_session.invoice(1000,"ciao","bao")