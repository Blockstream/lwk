from lwk import *

mnemonic = Mnemonic.from_random(12)
network = Network.regtest_default()
signer = Signer(mnemonic, network)

master_blinding_key = signer.slip77_master_blinding_key()
fingerprint = signer.fingerprint()

path = DerivationPath.ss_path(network, "wpkh", 0)
assert str(path) == "84'/1'/0'"
assert str(DerivationPath.from_vec(path.to_vec())) == str(path)

# get xpub from signer, e.g. a Jade which connection is managed outside LWK
xpub = signer.keyorigin_xpub(Bip.new_bip84()).split("]")[1]
dpk = DescriptorPublicKey(f"[{fingerprint}/{path}]{xpub}")

# construct the descriptor from the obtained xpub
desc = WolletDescriptor.ss_desc_from_external_signer(
    network,
    "wpkh",
    0,  # bip32 account number
    master_blinding_key,
    dpk,
)

# Check against the descriptor obtained directly from the signer
d = signer.singlesig_desc(Singlesig.WPKH, DescriptorBlindingKey.SLIP77)
assert str(desc) == str(d)

# DescriptorPublicKey from_str/to_str roundtrip, with and without keyorigin
dpk = DescriptorPublicKey(xpub)
assert str(dpk) == xpub

keyorigin_xpub = signer.keyorigin_xpub(Bip.new_bip84())
dpk = DescriptorPublicKey(keyorigin_xpub)
assert str(dpk).replace("'", "h") == keyorigin_xpub
