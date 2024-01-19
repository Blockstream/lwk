default:
    just --list

build-python-bindings:
    LIBNAME=libks_bindings.${LIB_EXT} && cargo build --features bindings && cargo run --features bindings -- generate --library target/debug/${LIBNAME} --language python --out-dir target/debug/bindings && cp target/debug/${LIBNAME} target/debug/bindings

test-python-bindings: build-python-bindings
    PYTHONPATH=target/debug/bindings/ python3 -c 'import ks_bindings'

env-python-bindings: build-python-bindings
    PYTHONPATH=target/debug/bindings/ python3

build-docker:
    cd context && docker build . -t xenoky/ks-builder && cd -

push-docker: build-docker
    docker push xenoky/ks-builder # require credentials

kotlin-android: kotlin android

kotlin:
    LIBNAME=libks_bindings.${LIB_EXT} && cargo build --features bindings && cargo run --features bindings -- generate --library target/debug/${LIBNAME} --language kotlin --out-dir target/release/kotlin

android: aarch64-linux-android armv7-linux-androideabi i686-linux-android x86_64-linux-android
    cp -a target/release/kotlin/jniLibs ks_bindings/android_bindings/lib/src/main
    cp -a target/release/kotlin/ks_bindings ks_bindings/android_bindings/lib/src/main/kotlin

aarch64-linux-android:
	cargo ndk -t aarch64-linux-android -o target/release/kotlin/jniLibs build -p ks-bindings

armv7-linux-androideabi:
	cargo ndk -t armv7-linux-androideabi -o target/release/kotlin/jniLibs build -p ks-bindings

i686-linux-android:
	cargo ndk -t i686-linux-android -o target/release/kotlin/jniLibs build -p ks-bindings

x86_64-linux-android:
	cargo ndk -t x86_64-linux-android -o target/release/kotlin/jniLibs build -p ks-bindings
