from lwk import *

mnemonic = Mnemonic.from_random(12)
network = Network.regtest_default()
signer = Signer(mnemonic, network)

# TODO: signer.slip77_master_blinding_key
d = signer.singlesig_desc(Singlesig.WPKH, DescriptorBlindingKey.SLIP77)
master_blinding_key = str(d).split(",")[0].split("ct(")[1]
fingerprint = signer.fingerprint()

path = get_path(network, "wpkh", 0)
# get xpub from signer, e.g. a Jade which connection is managed outside LWK
xpub = signer.keyorigin_xpub(Bip.new_bip84()).split("]")[1]  # strip keyorigin

# construct the descriptor from the obtained xpub
desc = WolletDescriptor.from_xpub(
    network,
    "wpkh",
    0,  # bip32 account number
    master_blinding_key,
    fingerprint,
    xpub,
)

# Check against the descriptor obtained directly from the signer
assert str(desc) == str(d)
