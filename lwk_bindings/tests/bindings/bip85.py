from lwk import *

# ANCHOR: bip85
# Load mnemonic
test_mnemonic = "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about"
mnemonic = Mnemonic(test_mnemonic)

# Create signer
network = Network.testnet()
signer = Signer(mnemonic, network)

# Derive mnemonics
derived_0_12 = signer.derive_bip85_mnemonic(0, 12)
derived_0_24 = signer.derive_bip85_mnemonic(0, 24)
derived_1_12 = signer.derive_bip85_mnemonic(1, 12)
# ANCHOR_END: bip85