from lwk import *

# Using test mnemonic
test_mnemonic = "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about"
mnemonic = Mnemonic(test_mnemonic)

# Create network (regtest)
network = Network.regtest_default()

# Create signer
signer = Signer(mnemonic, network)

# Test BIP85 derivation with 12 words
derived_mnemonic_12 = signer.derive_bip85_mnemonic(0, 12)

# Test BIP85 derivation with 24 words
derived_mnemonic_24 = signer.derive_bip85_mnemonic(0, 24)

# Test that different indices produce different mnemonics
derived_mnemonic_1 = signer.derive_bip85_mnemonic(1, 12)
