from lwk import *

network = Network.testnet()

# Create the signer
signer = Signer.random(network)
mnemonic = str(signer.mnemonic())

# Connect and Login
signer_data = signer.amp0_signer_data()
amp0 = Amp0Connected(network, signer_data)
challenge = amp0.get_challenge()
sig = signer.amp0_sign_challenge(challenge)
amp0 = amp0.login(sig)

# Create AMP0 account
pointer = amp0.next_account()
xpub = signer.amp0_account_xpub(pointer)
amp_id = amp0.create_amp0_account(pointer, xpub)

# Create Watch-Only 
fingerprint = signer.keyorigin_xpub(Bip.new_bip49())[1:9]
username = f"user{fingerprint}"
password = f"pass{fingerprint}"

amp0.create_watch_only(username, password)

# Connect with AMP0 using Watch-Only
amp0wo = Amp0(network, username, password, amp_id)
assert amp0wo.address(None).index() > 20
