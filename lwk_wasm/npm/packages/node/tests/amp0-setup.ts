import { WebSocket } from "ws";
import * as lwk from "@blockstream/lwk-node";

function installWebSocketPolyfill(): void {
  (
    globalThis as typeof globalThis & {
      WebSocket: typeof globalThis.WebSocket;
    }
  ).WebSocket = WebSocket as unknown as typeof globalThis.WebSocket;
}

export async function runAmp0Setup(): Promise<void> {
  try {
    installWebSocketPolyfill();

    // ANCHOR: amp0-setup
    if (true) {
      // ANCHOR: ignore
      const hiddenMnemonic = "<mnemonic>";
      void hiddenMnemonic;
    } // ANCHOR: ignore
    const mnemonic = lwk.Mnemonic.fromRandom(12).toString(); // ANCHOR: ignore
    const m = new lwk.Mnemonic(mnemonic);
    const network = lwk.Network.testnet();
    const signer = new lwk.Signer(m, network);
    if (true) {
      // ANCHOR: ignore
      const hiddenUsername = "<username>";
      const hiddenPassword = "<password>";
      void hiddenUsername;
      void hiddenPassword;
    } // ANCHOR: ignore
    const username = `user${signer.fingerprint()}`; // ANCHOR: ignore
    const password = `pass${signer.fingerprint()}`; // ANCHOR: ignore

    // Collect signer data
    const signerData = signer.amp0SignerData();
    // Connect to AMP0
    const amp0connected = await lwk.Amp0Connected.connect(network, signerData);
    // Obtain and sign the authentication challenge
    const challenge = await amp0connected.getChallenge();
    const sig = signer.amp0SignChallenge(challenge);
    // Login
    const amp0loggedin = await amp0connected.login(sig);
    // Create a new AMP0 account
    const pointer = amp0loggedin.nextAccount();
    const accountXpub = signer.amp0AccountXpub(pointer);
    const ampId = await amp0loggedin.createAmp0Account(pointer, accountXpub);
    // Create watch only entries
    await amp0loggedin.createWatchOnly(username, password);
    // Use watch only credentials to interact with AMP0
    await lwk.Amp0.newWithNetwork(network, username, password, ampId);
    // ANCHOR_END: amp0-setup
    process.exit(0); // TODO: expose a way to close the websocket
  } catch (error) {
    console.error("AMP0 setup test failed:", error);
    process.exit(1);
  }
}
