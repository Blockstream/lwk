async function runAmp0Setup() {
    try {
        const WebSocket = require('ws');
        global.WebSocket = WebSocket;

        const lwk = require('lwk_node');

        // ANCHOR: amp0-setup
	if (true) { // ANCHOR: ignore
        const mnemonic = "<mnemonic>";
        } // ANCHOR: ignore
        const mnemonic = lwk.Mnemonic.fromRandom(12).toString(); // ANCHOR: ignore
        const m = new lwk.Mnemonic(mnemonic);
        const network = lwk.Network.testnet();
        const signer = new lwk.Signer(m, network);
	if (true) { // ANCHOR: ignore
        const username = "<username>";
        const password = "<password>";
        } // ANCHOR: ignore
        const username = "user" + signer.fingerprint(); // ANCHOR: ignore
        const password = "pass" + signer.fingerprint(); // ANCHOR: ignore

        // Collect signer data
        const signer_data = signer.amp0SignerData();
        // Connect to AMP0
        const amp0connected = await new lwk.Amp0Connected(network, signer_data);
        // Obtain and sign the authentication challenge
        const challenge = await amp0connected.getChallenge();
        const sig = signer.amp0SignChallenge(challenge);
        // Login
        const amp0loggedin = await amp0connected.login(sig);
        // Create a new AMP0 account
        const pointer = amp0loggedin.nextAccount();
        const account_xpub = signer.amp0AccountXpub(pointer);
        const amp_id = await amp0loggedin.createAmp0Account(pointer, account_xpub);
        // Create watch only entries
        await amp0loggedin.createWatchOnly(username, password);
        // Use watch only credentials to interact with AMP0
        const amp0 = await new lwk.Amp0(network, username, password, amp_id);
        // ANCHOR_END: amp0-setup
        process.exit(0); // TODO: expose a way to close the websocket
    } catch (error) {
        console.error("AMP0 test failed:", error);
        process.exit(1);
    }
}

if (require.main === module) {
    runAmp0Setup();
}

module.exports = { runAmp0Setup };
