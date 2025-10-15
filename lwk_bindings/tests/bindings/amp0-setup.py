from lwk import *

# ANCHOR: amp0-setup
# Create signer and watch only credentials
username = "<username>";
password = "<password>";
mnemonic = "<mnemonic>";

mnemonic = Mnemonic.from_random(12); # ANCHOR: ignore
network = Network.testnet()
signer = Signer(mnemonic, network)
username = f"user{signer.fingerprint()}" # ANCHOR: ignore
password = f"pass{signer.fingerprint()}" # ANCHOR: ignore

# Collect signer data
signer_data = signer.amp0_signer_data();

# Connect to AMP0
amp0 = Amp0Connected(network, signer_data);

# Obtain and sign the authentication challenge
challenge = amp0.get_challenge();
sig = signer.amp0_sign_challenge(challenge);

# Login
amp0 = amp0.login(sig);

# Create a new AMP0 account
pointer = amp0.next_account();
account_xpub = signer.amp0_account_xpub(pointer);
amp_id = amp0.create_amp0_account(pointer, account_xpub);

# Create watch only entries
amp0.create_watch_only(username, password);

# Use watch only credentials to interact with AMP0
amp0 = Amp0(network, username, password, amp_id);
# ANCHOR_END: amp0-setup
