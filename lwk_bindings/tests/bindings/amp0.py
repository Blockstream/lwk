from lwk import *

# AMP0 credentials
username = "userleo34567"
password = "userleo34567"
# AMP ID
amp_id = ""

# BIP39 mnemonic corresponding to the AMP0 account
mnemonic = Mnemonic("idea bind tissue wood february mention unable collect expand stuff snap stock")

network = Network.testnet()
url = "https://waterfalls.liquidwebwallet.org/liquidtestnet/api"

# Create AMP0 object
amp0 = Amp0(network, username, password, amp_id)

# Get an address
addr1 = str(amp0.address(1).address())
assert addr1 == 'vjTvpDMQx3EQ2bS3pmmy7RivU3QTjGyyJFJy1Y5basdKmwpW3R4YRdsxFNT7B3bPNmJkgKCRCS63AtjR'
