from lwk import *
import os


gitlab_ci = os.environ.get("GITLAB_CI")
ci_job_name = os.environ.get("CI_JOB_NAME")
ci_branch_name = os.environ.get("CI_COMMIT_BRANCH")
if gitlab_ci == "true":
    # We are in a CI job
    if ci_branch_name != "master" or ci_job_name != "amp0":
        print("Skipping test")
        quit()


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
