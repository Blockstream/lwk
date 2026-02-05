import * as lwk from "lwk_node";
import WebSocket from "ws";

describe("AMP0Setup", function () {
  before(function () {
    (global as unknown as { WebSocket: typeof WebSocket }).WebSocket = WebSocket;
  });

  it("should setup AMP0 account", async function () {
    // ANCHOR: amp0-setup
    if (true) {
      // ANCHOR: ignore
      const mnemonic = "<mnemonic>";
    } // ANCHOR: ignore
    const mnemonic = lwk.Mnemonic.fromRandom(12).toString(); // ANCHOR: ignore
    const m = new lwk.Mnemonic(mnemonic);
    const network = lwk.Network.testnet();
    const signer = new lwk.Signer(m, network) as any;
    if (true) {
      // ANCHOR: ignore
      const username = "<username>";
      const password = "<password>";
    } // ANCHOR: ignore
    const username = "user" + signer.fingerprint(); // ANCHOR: ignore
    const password = "pass" + signer.fingerprint(); // ANCHOR: ignore

    // Collect signer data
    const signer_data = signer.amp0SignerData();
    // Connect to AMP0
    const amp0connected = new lwk.Amp0Connected(network, signer_data);
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
    const amp0 = lwk.Amp0.newTestnet(username, password, amp_id);
    // ANCHOR_END: amp0-setup
  });
});
