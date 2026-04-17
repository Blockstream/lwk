const assert = require('assert');
const fs = require('fs');
const lwk = require('lwk_node');

function createStorage() {
    const store = new Map();
    return {
        get(key) {
            return store.get(key) || null;
        },
        put(key, value) {
            const valueCopy = value ? new Uint8Array(value) : null;
            store.set(key, valueCopy);
        },
        remove(key) {
            store.delete(key);
        },
        _data: store
    };
}

function runWolletBuilderStoreTest() {
    const descriptorString = fs
        .readFileSync(`${__dirname}/../../test_data/update_with_mnemonic/descriptor.txt`, 'utf8')
        .trim();
    const encryptedUpdate = fs
        .readFileSync(
            `${__dirname}/../../test_data/update_with_mnemonic/update_serialized_encrypted.txt`,
            'utf8'
        )
        .trim();

    const network = lwk.Network.testnet();
    const descriptor = new lwk.WolletDescriptor(descriptorString);
    const update = lwk.Update.deserializeDecryptedBase64(encryptedUpdate, descriptor);
    const jsStorage = createStorage();

    const wollet = new lwk.WolletBuilder(network, descriptor)
        .withStore(jsStorage)
        .build();
    wollet.applyUpdate(update);

    assert(jsStorage._data.has('000000000000'));

    const expectedAddress = wollet.address(0).address().toString();
    const expectedTransactions = wollet.transactions().length;
    const expectedBalance = JSON.stringify(wollet.balance().toJSON());

    const restored = new lwk.WolletBuilder(network, descriptor)
        .withStore(jsStorage)
        .build();

    assert.strictEqual(restored.address(0).address().toString(), expectedAddress);
    assert.strictEqual(1, expectedTransactions);
    assert.strictEqual(restored.transactions().length, expectedTransactions);
    assert.strictEqual(JSON.stringify(restored.balance().toJSON()), expectedBalance);
}

if (require.main === module) {
    runWolletBuilderStoreTest();
    console.log("wollet_builder.js: all tests passed");
}

module.exports = { runWolletBuilderStoreTest };
