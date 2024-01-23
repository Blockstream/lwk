default:
    just --list

build-python-bindings:
    LIBNAME=liblwk_bindings.${LIB_EXT} && cargo build --features bindings && cargo run --features bindings -- generate --library target/debug/${LIBNAME} --language python --out-dir target/debug/bindings && cp target/debug/${LIBNAME} target/debug/bindings

test-python-bindings: build-python-bindings
    PYTHONPATH=target/debug/bindings/ python3 -c 'import lwk_bindings'

env-python-bindings: build-python-bindings
    PYTHONPATH=target/debug/bindings/ python3

build-docker:
    cd context && docker build . -t xenoky/lwk-builder && cd -

push-docker: build-docker
    docker push xenoky/lwk-builder # require credentials

kotlin-android: kotlin android

kotlin:
    LIBNAME=liblwk_bindings.${LIB_EXT} && cargo build --features bindings && cargo run --features bindings -- generate --library target/debug/${LIBNAME} --language kotlin --out-dir target/release/kotlin

android: aarch64-linux-android armv7-linux-androideabi i686-linux-android x86_64-linux-android
    cp -a target/release/kotlin/jniLibs lwk_bindings/android_bindings/lib/src/main
    cp -a target/release/kotlin/lwk_bindings lwk_bindings/android_bindings/lib/src/main/kotlin

aarch64-linux-android:
	cargo ndk -t aarch64-linux-android -o target/release/kotlin/jniLibs build -p lwk-bindings

armv7-linux-androideabi:
	cargo ndk -t armv7-linux-androideabi -o target/release/kotlin/jniLibs build -p lwk-bindings

i686-linux-android:
	cargo ndk -t i686-linux-android -o target/release/kotlin/jniLibs build -p lwk-bindings

x86_64-linux-android:
	cargo ndk -t x86_64-linux-android -o target/release/kotlin/jniLibs build -p lwk-bindings
