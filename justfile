default:
    just --list

build-bindings:
    LIBNAME=libks_bindings.so && cargo build --features bindings && cargo run --features bindings -- generate --library target/debug/${LIBNAME} --language python --out-dir target/debug/bindings && cp target/debug/${LIBNAME} target/debug/bindings

test-bindings: build-bindings
    PYTHONPATH=target/debug/bindings/ python3 -c 'import ks_bindings'

env-bindings: build-bindings
    PYTHONPATH=target/debug/bindings/ python3