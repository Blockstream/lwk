const assert = require('assert');
const fs = require('fs');
const lwk = require('lwk_node');

function runAddDetailsTest() {
    const desc = fs
        .readFileSync(`${__dirname}/../../test_data/pset_details/desc`, 'utf8')
        .trim();
    const psetBase64 = fs
        .readFileSync(`${__dirname}/../../test_data/pset_details/pset.base64`, 'utf8')
        .trim();

    const network = lwk.Network.testnet();
    const descriptor = new lwk.WolletDescriptor(desc);
    const wollet = new lwk.Wollet(network, descriptor);
    const pset = new lwk.Pset(psetBase64);

    const before = pset.toString();
    pset.addDetails(wollet);
    const after = pset.toString();

    assert.notStrictEqual(after, before, 'addDetails should enrich the PSET');
    assert(
        after.length > before.length,
        'addDetails should increase the serialized PSET length by adding data'
    );

    pset.addDetails(wollet);
    assert.strictEqual(
        pset.toString(),
        after,
        'addDetails should be idempotent once the PSET has been enriched'
    );
}

if (require.main === module) {
    runAddDetailsTest();
}

module.exports = { runAddDetailsTest };
