import { WebSocket } from "ws";
import * as lwk from "@blockstream/lwk-node";

function installWebSocketPolyfill(): void {
  (
    globalThis as typeof globalThis & {
      WebSocket: typeof globalThis.WebSocket;
    }
  ).WebSocket = WebSocket as unknown as typeof globalThis.WebSocket;
}

export async function runAmp0DailyOps(): Promise<void> {
  try {
    const ciBranchName = process.env.CI_COMMIT_BRANCH;
    if (ciBranchName !== undefined && ciBranchName !== "master") {
      console.log("Skipping test");
      process.exit(0);
    }

    installWebSocketPolyfill();

    // ANCHOR: amp0-daily-ops
    if (true) {
      // ANCHOR: ignore
      const hiddenMnemonic = "<mnemonic>";
      void hiddenMnemonic;
    } // ANCHOR: ignore
    const mnemonic =
      "thrive metal cactus come oval candy medal bounce captain shock permit joke"; // ANCHOR: ignore
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
    const username = "userlwk001"; // ANCHOR: ignore
    const password = "userlwk001"; // ANCHOR: ignore
    const ampId = "";

    // Create AMP0 object
    const amp0 = await lwk.Amp0.newTestnet(username, password, ampId);

    // Get an address
    const addrResult = await amp0.address(1);

    // Create wollet
    const wollet = amp0.wollet();

    // Sync the wallet
    const url = "https://waterfalls.liquidwebwallet.org/liquidtestnet/api";
    const client = new lwk.EsploraClient(network, url, true, 4, false);

    const lastIndex = amp0.lastIndex();
    const update = await client.fullScanToIndex(wollet, lastIndex);
    if (update) {
      wollet.applyUpdate(update);
    }

    // Get the wallet transactions
    const txs = wollet.transactions();
    void txs;

    // Get the balance
    const balanceEntries = new Map<string, bigint>(wollet.balance().entries());
    const lbtc = network.policyAsset(); // ANCHOR: ignore
    const lbtcBalance = balanceEntries.get(lbtc.toString()) ?? 0n; // ANCHOR: ignore
    if (lbtcBalance < 500n) {
      // ANCHOR: ignore
      const addr = addrResult.address().toString(); // ANCHOR: ignore
      console.log(
        `Balance is insufficient to make a transaction, send some tLBTC to ${addr}`
      ); // ANCHOR: ignore
      return; // ANCHOR: ignore
    } // ANCHOR: ignore

    // Create a (redeposit) transaction
    let builder = network.txBuilder();
    builder = builder.drainLbtcWallet();
    const amp0pset = builder.finishForAmp0(wollet);

    // Sign with the user key
    const pset = amp0pset.pset();
    const signedPset = signer.sign(pset);

    // Ask AMP0 to cosign
    const amp0psetSigned = new lwk.Amp0Pset(
      signedPset,
      amp0pset.blindingNonces()
    );
    const tx = await amp0.sign(amp0psetSigned);

    // Broadcast
    const txid = await client.broadcastTx(tx);
    // ANCHOR_END: amp0-daily-ops
    console.log(txid.toString());
    process.exit(0); // TODO: expose a way to close the websocket
  } catch (error) {
    console.error("AMP0 daily ops test failed:", error);
    process.exit(1);
  }
}
