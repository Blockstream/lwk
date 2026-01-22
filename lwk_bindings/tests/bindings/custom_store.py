from lwk import *


class PythonStore(ForeignStore):
    """A simple in-memory store implementation in Python."""

    def __init__(self):
        self.data = {}

    def get(self, key):
        return self.data.get(key)

    def put(self, key, value):
        self.data[key] = value

    def delete(self, key):
        self.data.pop(key, None)


# Create test helper wrapping the Python store
py_store = PythonStore()
store = ForeignStoreLink(py_store)
test = LwkTestStore(store)

# Test Rust writing to Python store
test.write("key", b"value")
assert py_store.data["key"] == b"value", "Rust write should appear in Python store"

# Test Rust reading from Python store
result = test.read("key")
assert result == b"value", "Rust should read what was written"

# Test Rust reading non-existent key
result = test.read("nonexistent")
assert result is None, "Non-existent key should return None"

# Test Rust overwriting
test.write("key", b"new_value")
result = test.read("key")
assert result == b"new_value", "Rust should read updated value"

# Test Rust deleting
test.delete("key")
result = test.read("key")
assert result is None, "Deleted key should return None"

# Test delete non-existent key (should not raise)
test.delete("key")

# Test with namespaced keys (as intended for LWK usage)
test.write("Liquid:Tx:abc123", b"tx_data")
test.write("Liquid:Addr:0", b"addr_data")
assert test.read("Liquid:Tx:abc123") == b"tx_data"
assert test.read("Liquid:Addr:0") == b"addr_data"

print("custom_store.py: all tests passed")
