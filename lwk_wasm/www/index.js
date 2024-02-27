import * as lwk from "lwk_wasm";

const balance = document.getElementById("balance")
const scanButton = document.getElementById("scan-button")
const descriptor = document.getElementById("descriptor")

scanButton.disabled = false;  // The button start disabled and it's enabled here once the wasm has been loaded
scanButton.addEventListener("click", scanButtonPressed);

async function scanButtonPressed(e) {
    try {
        balance.innerText = "Loading... Open the browser dev tools to see network calls..."
        scanButton.disabled = true;
        let desc = descriptor.value

        // This is hacky...
        let network = desc.includes("xpub") ? lwk.Network.mainnet() : lwk.Network.testnet()

        let client = network.defaultEsploraClient()
        let wolletDescriptor = new lwk.WolletDescriptor(desc)
        let wollet = new lwk.Wollet(network, wolletDescriptor)
        let update = await client.fullScan(wollet)
        wollet.applyUpdate(update)
        let val = wollet.balance();
        let balanceString = JSON.stringify(Object.fromEntries(val), null, 1)
            .replace("6f0279e9ed041c3d710a9f57d0c02928416460c4b722ae3457a11eec381c526d", "L-BTC")
            .replace("144c654344aa716d6f3abcc1ca90e5641e4e2a7f633bc09fe3baf64585819a49", "tL-BTC")
        balance.innerText = balanceString
    } catch (e) {
        balance.innerText = e
    } finally {
        scanButton.disabled = false;
    }
}