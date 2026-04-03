from lwk import *

mnemonic = Mnemonic("abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about")
network = Network.regtest_default()

expected_desc = "ct(d985e9b3bda4d5d3f5c43cd6acdc625d89b4a0aca592220b30cfeb9ca48eaef6,elwsh(multi(2,[3d970d04/87'/1'/0']tpubDC347GyKEGtyd4swZDaEmBTcNuqseyX7E3Yw58FoeV1njuBcUmBMr5vBeBh6eRsxKYHeCAEkKj8J2p2dBQQJwB8n33uyAPrdgwFxLFTCXRd/<0;1>/*,[73c5da0a/87'/1'/0']tpubDCChhoz5Qdrkn7Z7KXawq6Ad6r3A4MUkCoVTqeWxfTkA6bHNJ3CHUEtALQdkNeixNz4446PcAmw4WKcj3mV2vb29H7sg9EPzbyCU1y2merw/<0;1>/*)))#0aq2fx4y"
signer = Signer(mnemonic, network)

amp2 = Amp2.new_testnet()
xpub = signer.keyorigin_xpub(Bip.new_bip87())
desc = amp2.descriptor_from_str(xpub)
assert str(desc.descriptor()) == expected_desc

server_key = "[3d970d04/87'/1'/0']tpubDC347GyKEGtyd4swZDaEmBTcNuqseyX7E3Yw58FoeV1njuBcUmBMr5vBeBh6eRsxKYHeCAEkKj8J2p2dBQQJwB8n33uyAPrdgwFxLFTCXRd"
url = "http://127.0.0.1:5000"
amp2 = Amp2(server_key, url)
desc = amp2.descriptor_from_str(xpub)
assert str(desc.descriptor()) == expected_desc
