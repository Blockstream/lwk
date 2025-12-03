from lwk import *

# Values from ELIP152
desc_main = "ct(3e129856c574c66d94023ac98b7f69aca9774d10aee4dc087f0c52a498687189,elwpkh([73c5da0a/84h/1776h/0h]xpub6CRFzUgHFDaiDAQFNX7VeV9JNPDRabq6NYSpzVZ8zW8ANUCiDdenkb1gBoEZuXNZb3wPc1SVcDXgD2ww5UBtTb8s8ArAbTkoRQ8qn34KgcY/0/*))"
desc_test = "ct(3e129856c574c66d94023ac98b7f69aca9774d10aee4dc087f0c52a498687189,elwpkh([73c5da0a/84h/1h/0h]tpubDC8msFGeGuwnKG9Upg7DM2b4DaRqg3CUZa5g8v2SRQ6K4NSkxUgd7HsL2XVWbVm39yBA4LAxysQAm397zwQSQoQgewGiYZqrA9DsP4zbQ1M/0/*))"

wollet_main = Wollet(Network.mainnet(), WolletDescriptor(desc_main), datadir=None)
wollet_test = Wollet(Network.testnet(), WolletDescriptor(desc_test), datadir=None)
wollet_regt = Wollet(Network.regtest_default(), WolletDescriptor(desc_test), datadir=None)

assert wollet_main.dwid() == "b781-7bc7-db64-c3de-3937-7eb7-c9ab-f799"
assert wollet_test.dwid() == "977a-c955-9289-b81a-e5b1-1ef9-f903-ec8a"
assert wollet_regt.dwid() == "f4b6-a53f-e7c1-02be-920c-a558-ff83-5979"
