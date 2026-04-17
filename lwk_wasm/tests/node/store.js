const assert = require('assert');
const fs = require('fs');
const lwk = require('lwk_node');

function createStorage(isPersisted = false) {
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
        isPersisted() {
            return isPersisted;
        },
        _data: store
    };
}

function storageSize(storage) {
    let size = 0;
    for (const [key, value] of storage._data.entries()) {
        size += Buffer.byteLength(key, 'utf8');
        if (value) {
            size += value.byteLength;
        }
    }
    return size;
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
    const jsStorage = createStorage(false);

    const wollet = new lwk.WolletBuilder(network, descriptor)
        .withExperimentalStore(jsStorage)
        .build();
    wollet.applyUpdate(update);

    assert(jsStorage._data.has('000000000000'));
    const jsStorageWithoutTxsStoreSize = storageSize(jsStorage);

    const expectedAddress = wollet.address(0).address().toString();
    const expectedTransactions = wollet.transactions().length;
    const expectedBalance = JSON.stringify(wollet.balance().toJSON());

    const restored = new lwk.WolletBuilder(network, descriptor)
        .withExperimentalStore(jsStorage)
        .build();

    assert.strictEqual(restored.address(0).address().toString(), expectedAddress);
    assert.strictEqual(1, expectedTransactions);
    assert.strictEqual(restored.transactions().length, expectedTransactions);
    assert.strictEqual(JSON.stringify(restored.balance().toJSON()), expectedBalance);

    const jsStorageWithTxsStore = createStorage(false);
    const txsStorage = createStorage(true);
    const wolletWithTxsStore = new lwk.WolletBuilder(network, descriptor)
        .withExperimentalStore(jsStorageWithTxsStore)
        .withTxsStore(txsStorage, false)
        .build();
    wolletWithTxsStore.applyUpdate(update);

    assert(jsStorageWithTxsStore._data.has('000000000000'));
    assert(txsStorage._data.has('wollet:txids'));
    assert(
        storageSize(jsStorageWithTxsStore) < jsStorageWithoutTxsStoreSize,
        'jsStorage should be smaller when transactions are stored separately'
    );
}

if (require.main === module) {
    runWolletBuilderStoreTest();
    console.log("wollet_builder.js: all tests passed");
}

module.exports = { runWolletBuilderStoreTest };
