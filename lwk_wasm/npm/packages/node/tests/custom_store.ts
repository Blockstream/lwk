import * as assert from "node:assert/strict";
import * as lwk from "lwk_node";

type StorageLike = {
    _data: Map<string, Uint8Array | null>;
    get(key: string): Uint8Array | null;
    put(key: string, value: Uint8Array | null): void;
    remove(key: string): void;
};

/**
 * A simple in-memory storage implementation in JavaScript.
 * This demonstrates the duck-typed storage pattern from the RFC.
 */
function createStorage(): StorageLike {
    const store = new Map<string, Uint8Array | null>();

    return {
        get(key: string) {
            return store.get(key) ?? null;
        },
        put(key: string, value: Uint8Array | null) {
            // IMPORTANT: Copy the value! WASM may reuse the underlying memory buffer.
            const valueCopy = value ? new Uint8Array(value) : null;
            store.set(key, valueCopy);
        },
        remove(key: string) {
            store.delete(key);
        },
        // Helper for testing - not required by the interface
        _data: store,
    };
}

export default async function runCustomStoreTest() {
    // Create a JavaScript storage object
    const jsStorage = createStorage();

    // Create the test helper wrapping the JS storage
    const test = new lwk.JsTestStore(jsStorage);

    // Test Rust writing to JS storage
    test.write("key", new Uint8Array([118, 97, 108, 117, 101])); // "value"
    const storedValue = jsStorage._data.get("key");
    assert.ok(storedValue !== undefined, "Rust write should appear in JS storage");

    // Test Rust reading from JS storage
    let result = test.read("key");
    assert.ok(result !== null, "Should read back the value");
    assert.ok(
        arrayEquals(result, new Uint8Array([118, 97, 108, 117, 101])),
        "Rust should read what was written"
    );

    // Test Rust reading non-existent key
    result = test.read("nonexistent");
    // wasm-bindgen may return undefined instead of null for Option::None
    assert.ok(result === null || result === undefined, "Non-existent key should return null/undefined");

    // Test Rust overwriting
    test.write("key", new Uint8Array([110, 101, 119])); // "new"
    result = test.read("key");
    assert.ok(
        arrayEquals(result, new Uint8Array([110, 101, 119])),
        "Rust should read updated value"
    );

    // Test Rust removing
    test.remove("key");
    result = test.read("key");
    assert.ok(result === null || result === undefined, "Removed key should return null/undefined");

    // Test remove non-existent key (should not throw)
    test.remove("key");

    // Test with namespaced keys (as intended for LWK usage)
    test.write("Liquid:Tx:abc123", new Uint8Array([1, 2, 3]));
    test.write("Liquid:Addr:0", new Uint8Array([4, 5, 6]));

    result = test.read("Liquid:Tx:abc123");
    assert.ok(arrayEquals(result, new Uint8Array([1, 2, 3])), "Should read namespaced key");

    result = test.read("Liquid:Addr:0");
    assert.ok(arrayEquals(result, new Uint8Array([4, 5, 6])), "Should read another namespaced key");

    console.log("custom_store.ts: all tests passed");
}

function arrayEquals(a: any, b: any) {
    if (a === null || a === undefined || b === null || b === undefined) {
        return (a === null || a === undefined) && (b === null || b === undefined);
    }
    // Convert to Uint8Array if needed (wasm-bindgen may return different array types)
    const arrA = a instanceof Uint8Array ? a : new Uint8Array(a);
    const arrB = b instanceof Uint8Array ? b : new Uint8Array(b);
    if (arrA.length !== arrB.length) return false;
    for (let i = 0; i < arrA.length; i++) {
        if (arrA[i] !== arrB[i]) return false;
    }
    return true;
}
