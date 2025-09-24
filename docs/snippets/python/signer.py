# ANCHOR: generate-signer
from lwk import *

mnemonic = Mnemonic.from_random(12)
network = Network.testnet()

signer = Signer(mnemonic, network)
# ANCHOR_END: generate-signer
