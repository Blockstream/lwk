default:
    just --list

build-bindings:
    LIBNAME=libks_bindings.so && cargo build && cargo run -- generate --library target/debug/${LIBNAME} --language python --out-dir target/debug/bindings && cp target/debug/${LIBNAME} target/debug/bindings

test-bindings: build-bindings
    PYTHONPATH=target/debug/bindings/ python3 -c 'import keystone; print(keystone.hello())'

env-bindings: build-bindings
    PYTHONPATH=target/debug/bindings/ python3