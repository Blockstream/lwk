const lwk = require('lwk_node');
const assert = require('node:assert/strict');

const createdSources = [];

class TestEventSource {
    constructor(url) {
        this.url = url;
        this.listeners = {};
        this.controller = new AbortController();
        this.closed = false;
        this.bytesRead = 0;
        this.ready = new Promise((resolve, reject) => {
            this.resolveReady = resolve;
            this.rejectReady = reject;
        });
        this.run();
        createdSources.push(this);
    }

    addEventListener(type, listener) {
        this.listeners[type] = listener;
    }

    removeEventListener(type) {
        delete this.listeners[type];
    }

    close() {
        this.closed = true;
        this.controller.abort();
    }

    async run() {
        try {
            const response = await fetch(this.url, { signal: this.controller.signal });
            if (!response.ok) {
                throw new Error(`Subscribe request failed with status ${response.status}`);
            }
            await this.readEvents(response.body);
        } catch (error) {
            if (error.name === 'AbortError') {
                return;
            }
            this.rejectReady(error);
            if (this.onerror) {
                this.onerror(error);
            }
        }
    }

    async readEvents(body) {
        const reader = body.getReader();
        const decoder = new TextDecoder();
        let buffer = "";

        while (!this.controller.signal.aborted) {
            const { done, value } = await reader.read();
            if (done) {
                break;
            }
            this.bytesRead += value.length;
            this.resolveReady();

            buffer += decoder.decode(value, { stream: true });
            const chunks = buffer.split("\n\n");
            buffer = chunks.pop();

            for (const chunk of chunks) {
                this.dispatch(chunk);
            }
        }
    }

    dispatch(chunk) {
        const event = chunk
            .split("\n")
            .find((line) => line.startsWith("event:"))
            ?.slice("event:".length)
            .trim();
        const data = chunk
            .split("\n")
            .filter((line) => line.startsWith("data:"))
            .map((line) => line.slice("data:".length).trimStart())
            .join("\n");

        if (event && this.listeners[event]) {
            this.listeners[event]({ data });
        }
    }
}

globalThis.EventSource = TestEventSource;

async function withTimeout(promise, ms) {
    let timeout;
    try {
        return await Promise.race([
            promise,
            new Promise((_, reject) => {
                timeout = setTimeout(() => reject(new Error(`Timed out after ${ms}ms`)), ms);
            }),
        ]);
    } finally {
        clearTimeout(timeout);
    }
}

async function runSubscribeTest() {
    try {
        console.log("Starting subscribe test");

        const mnemonic = new lwk.Mnemonic("uncle win diagram apple poverty sun cement rib opera barely april mountain");
        const network = lwk.Network.testnet();
        const signer = new lwk.Signer(mnemonic, network);
        const desc = signer.wpkhSlip77Descriptor();
        const wollet = new lwk.Wollet(network, desc);

        const url = "https://waterfalls.liquidwebwallet.org/liquidtestnet/api";
        const client = new lwk.WaterfallsClient(network, url);
        const updates = [];
        const errors = [];
        let waitForBroadcastMempool = false;
        let resolveMempoolUpdate;
        const mempoolUpdate = new Promise((resolve) => {
            resolveMempoolUpdate = resolve;
        });

        const subscription = await client.subscribe(
            desc,
            (kind, rawData) => {
                const data = JSON.parse(rawData);
                updates.push({ kind, rawData, data });
                if (kind === "mempool" && waitForBroadcastMempool) {
                    resolveMempoolUpdate(data);
                }
            },
            (error) => {
                errors.push(error);
            },
        );

        assert.equal(createdSources.length, 1);
        assert.match(createdSources[0].url, /^https:\/\/waterfalls\.liquidwebwallet\.org\/liquidtestnet\/api\/v1\/subscribe\?descriptor=/);

        await withTimeout(createdSources[0].ready, 30_000);
        assert(createdSources[0].bytesRead > 0);
        assert.equal(errors.length, 0);

        const update = await client.fullScan(wollet);
        if (update) {
            wollet.applyUpdate(update);
        }
        const balance = wollet.balance().toJSON();
        assert(balance[network.policyAsset().toString()] > 1_000);

        const sats = BigInt(1_000);
        const address = wollet.address(null).address();
        const asset = network.policyAsset();

        let builder = new lwk.TxBuilder(network);
        builder = builder.addRecipient(address, sats, asset);
        let pset = builder.finish(wollet);
        pset = signer.sign(pset);
        pset = wollet.finalize(pset);
        const tx = pset.extractTx();
        waitForBroadcastMempool = true;
        const txid = await client.broadcastTx(tx);
        assert(txid.toString().length > 0);

        const mempoolData = await withTimeout(mempoolUpdate, 60_000);
        assert.equal(mempoolData.type, "mempool");
        assert(mempoolData.tip);

        wollet.applyTransaction(tx);

        for (const update of updates) {
            assert(["tip", "mempool", "block", "reorg"].includes(update.kind));
            assert.equal(update.data.type, update.kind);
        }

        subscription.close();
        assert.equal(createdSources[0].closed, true);

        console.log("Subscribe test passed!");
    } catch (error) {
        console.error("Subscribe test failed:", error);
        throw error;
    }
}

if (require.main === module) {
    runSubscribeTest();
}

module.exports = { runSubscribeTest };
