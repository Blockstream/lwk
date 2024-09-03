default:
    just --list

python-build-bindings:
    LIBNAME=liblwk.${LIB_EXT} && cargo build --features bindings && cargo run --features bindings -- generate --library target/debug/${LIBNAME} --language python --out-dir target/debug/bindings && cp target/debug/${LIBNAME} target/debug/bindings

python-test-bindings: python-build-bindings
    PYTHONPATH=target/debug/bindings/ python3 -c 'import lwk'

python-env-bindings: python-build-bindings
    PYTHONPATH=target/debug/bindings/ python3

docker-build:
    cd context && docker build . -t xenoky/lwk-builder && cd -

docker-push: docker-build
    docker push xenoky/lwk-builder # require credentials

kotlin-android: kotlin android

kotlin:
    LIBNAME=liblwk.${LIB_EXT} && cargo build --features bindings && cargo run --features bindings -- generate --library target/debug/${LIBNAME} --language kotlin --out-dir target/release/kotlin

android: aarch64-linux-android armv7-linux-androideabi i686-linux-android x86_64-linux-android
    cp -a target/release/kotlin/jniLibs lwk_bindings/android_bindings/lib/src/main
    cp -a target/release/kotlin/lwk_bindings lwk_bindings/android_bindings/lib/src/main/kotlin

aarch64-linux-android:
	cargo ndk -t aarch64-linux-android -o target/release/kotlin/jniLibs build -p lwk_bindings

armv7-linux-androideabi:
	cargo ndk -t armv7-linux-androideabi -o target/release/kotlin/jniLibs build -p lwk_bindings

i686-linux-android:
	cargo ndk -t i686-linux-android -o target/release/kotlin/jniLibs build -p lwk_bindings

x86_64-linux-android:
	cargo ndk -t x86_64-linux-android -o target/release/kotlin/jniLibs build -p lwk_bindings

swift: ios ios-sim
    cargo run --features bindings -- generate --library ./target/aarch64-apple-ios/release/liblwk.a --language swift --out-dir ./target/swift
    mkdir -p ./target/swift/include
    mv target/swift/lwkFFI.h target/swift/include
    mv target/swift/lwkFFI.modulemap  target/swift/include/module.modulemap
    xcodebuild -create-xcframework -library target/lipo-ios-sim/release/liblwk.a -headers target/swift/include -library target/aarch64-apple-ios/release/liblwk.a -headers target/swift/include -output target/lwkFFI.xcframework

ios: aarch64-apple-ios

ios-sim: x86_64-apple-ios aarch64-apple-ios-sim
    mkdir -p target/lipo-ios-sim/release
    lipo target/aarch64-apple-ios-sim/release/liblwk.a target/x86_64-apple-ios/release/liblwk.a -create -output target/lipo-ios-sim/release/liblwk.a

x86_64-apple-ios:
    cargo build --release --target x86_64-apple-ios -p lwk_bindings

aarch64-apple-ios:
    cargo build --release --target aarch64-apple-ios -p lwk_bindings

aarch64-apple-ios-sim:
    cargo build --release --target aarch64-apple-ios-sim -p lwk_bindings
