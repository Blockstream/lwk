# list available recipes
default:
    just --list

SIMPLICITY_FEATURES := if env_var_or_default("SIMPLICITY", "") != "" { "--features simplicity" } else { "" }

# build the bindings lib: liblwk.so (as specified in lwk_bindings/Cargo.toml)
build-bindings-lib:
    # a debug build would be fine if used only to generate interfaces files but some jobs use it to package it, thus release is necessary.
    cargo build --release -p lwk_bindings {{SIMPLICITY_FEATURES}}

# build the python interface "lwk.py"
python-build-bindings: build-bindings-lib
    # note production wheels are built with maturin, but this can be useful during development
    # we use release to generate interfaces only because build-bindings-lib is done in release, so we find intermediate packages
    cargo run --release --features bindings -- generate --library target/release/liblwk.so --language python --out-dir target/release/bindings
    cp target/release/liblwk.so target/release/bindings

# smoke test the python bindings
python-test-bindings: python-build-bindings
    PYTHONPATH=target/release/bindings/ python3 -c 'import lwk'

# build the bindings lib with simplicity feature
build-bindings-lib-simplicity:
    cargo build --release -p lwk_bindings --features simplicity

# build the python bindings with simplicity enabled
python-build-bindings-simplicity: build-bindings-lib-simplicity
    cargo run --release --features bindings,simplicity -- generate --library target/release/liblwk.so --language python --out-dir target/release/bindings
    cp target/release/liblwk.so target/release/bindings

# smoke test the python bindings with simplicity
python-test-bindings-simplicity: python-build-bindings-simplicity
    PYTHONPATH=target/release/bindings/ python3 lwk_bindings/tests/bindings/simplicity_p2pk.py
    PYTHONPATH=target/release/bindings/ python3 lwk_bindings/tests/bindings/simplicity_p2pk_regtest.py

# build the python bindings and start a python env with them
python-env-bindings: python-build-bindings
    PYTHONPATH=target/release/bindings/ python3

# build the docker "xenoky/lwk-builder" used in the CI
docker-build:
    cd context && docker build . -t xenoky/lwk-builder && cd -

# push the docker "xenoky/lwk-builder" on docker hub
docker-push: docker-build
    docker push xenoky/lwk-builder # require credentials

# build the docker "xenoky/lwk-nix-builder" used in the CI
docker-nix-build:
    docker build -f context/Dockerfile.nix . -t xenoky/lwk-nix-builder

# push the docker "xenoky/lwk-nix-builder" on docker hub
docker-nix-push: docker-nix-build
    docker push xenoky/lwk-nix-builder # require credentials

kotlin: build-bindings-lib
    cargo run --release --features bindings -- generate --library target/release/liblwk.so --language kotlin --out-dir target/release/kotlin
    cp -a target/release/kotlin/lwk lwk_bindings/android_bindings/lib/src/androidMain/kotlin

# Cross build the lib for aarch64-linux-android
aarch64-linux-android:
	cargo ndk -t aarch64-linux-android -o target/release/android/jniLibs build -p lwk_bindings {{SIMPLICITY_FEATURES}}

# Cross build the lib for armv7-linux-androideabi
armv7-linux-androideabi:
	cargo ndk -t armv7-linux-androideabi -o target/release/android/jniLibs build -p lwk_bindings {{SIMPLICITY_FEATURES}}

# Cross build the lib for i686-linux-android
i686-linux-android:
	cargo ndk -t i686-linux-android -o target/release/android/jniLibs build -p lwk_bindings {{SIMPLICITY_FEATURES}}

# Cross build the lib for x86_64-linux-android
x86_64-linux-android:
	cargo ndk -t x86_64-linux-android -o target/release/android/jniLibs build -p lwk_bindings {{SIMPLICITY_FEATURES}}

# After cross building all the lib for android put them in final dir
android: aarch64-linux-android armv7-linux-androideabi i686-linux-android x86_64-linux-android
    cp -a target/release/android/jniLibs lwk_bindings/android_bindings/lib/src/androidMain

# Build the kotlin multiplatform interface and android, ios and jvm
kotlin-multiplatform: ios ios-sim android jvm
    cargo install --bin gobley-uniffi-bindgen gobley-uniffi-bindgen@0.2.0
    gobley-uniffi-bindgen --config ./lwk_bindings/uniffi.kotlin-multiplatform.toml --library target/aarch64-apple-ios/release/liblwk.a  --out-dir target/release/kotlin-multiplatform
    cp -a target/release/kotlin-multiplatform/* lwk_bindings/android_bindings/lib/src/
    mkdir -p ./lwk_bindings/android_bindings/lib/src/libs/ios-arm64/
    mkdir -p ./lwk_bindings/android_bindings/lib/src/libs/ios-simulator-arm64/
    cp target/aarch64-apple-ios/release/liblwk.a lwk_bindings/android_bindings/lib/src/libs/ios-arm64/
    cp target/lipo-ios-sim/release/liblwk.a lwk_bindings/android_bindings/lib/src/libs/ios-simulator-arm64/

jvm: aarch64-apple-darwin # x86_64-unknown-linux-gnu
    mkdir -p lwk_bindings/android_bindings/lib/src/jvmMain/resources/darwin-aarch64
    cp -a target/aarch64-apple-darwin/release/liblwk.dylib lwk_bindings/android_bindings/lib/src/jvmMain/resources/darwin-aarch64/

# Build aarch64-apple-darwin
aarch64-apple-darwin:
    MACOSX_DEPLOYMENT_TARGET=11.0 cargo build --release --target aarch64-apple-darwin -p lwk_bindings {{SIMPLICITY_FEATURES}}

# Build x86_64-unknown-linux-gnu
x86_64-unknown-linux-gnu:
    cargo build --release --target x86_64-unknown-linux-gnu -p lwk_bindings {{SIMPLICITY_FEATURES}}

# Build ios (works only on mac)
ios: aarch64-apple-ios

# Build ios simulator libs x86/arm and merge them (works only on mac)
ios-sim: x86_64-apple-ios aarch64-apple-ios-sim
    mkdir -p target/lipo-ios-sim/release
    lipo target/aarch64-apple-ios-sim/release/liblwk.a target/x86_64-apple-ios/release/liblwk.a -create -output target/lipo-ios-sim/release/liblwk.a

# Build x86_64-apple-ios
x86_64-apple-ios:
    IPHONEOS_DEPLOYMENT_TARGET=12.0 MACOSX_DEPLOYMENT_TARGET=11.0 cargo build --release --target x86_64-apple-ios -p lwk_bindings {{SIMPLICITY_FEATURES}}

# Build aarch64-apple-ios
aarch64-apple-ios:
    IPHONEOS_DEPLOYMENT_TARGET=12.0 MACOSX_DEPLOYMENT_TARGET=11.0 cargo build --release --target aarch64-apple-ios -p lwk_bindings {{SIMPLICITY_FEATURES}}

# Build aarch64-apple-ios-sim
aarch64-apple-ios-sim:
    IPHONEOS_DEPLOYMENT_TARGET=12.0 MACOSX_DEPLOYMENT_TARGET=11.0 cargo build --release --target aarch64-apple-ios-sim -p lwk_bindings {{SIMPLICITY_FEATURES}}

# Build the swift framework (works only on mac)
swift: ios ios-sim
    # we are not using build-bindings-lib because we need the mac targets anyway
    cargo run --features bindings -- generate --library ./target/aarch64-apple-ios/release/liblwk.a --language swift --out-dir ./target/swift
    mkdir -p ./target/swift/include
    mv target/swift/lwkFFI.h target/swift/include
    mv target/swift/lwkFFI.modulemap  target/swift/include/module.modulemap
    xcodebuild -create-xcframework -library target/lipo-ios-sim/release/liblwk.a -headers target/swift/include -library target/aarch64-apple-ios/release/liblwk.a -headers target/swift/include -output target/lwkFFI.xcframework

csharp-windows: build-bindings-lib
    cargo install uniffi-bindgen-cs --git https://github.com/RCasatta/uniffi-bindgen-cs --rev fa87c381f88c8cacd26cf3e91e5c63af60162c3f
    uniffi-bindgen-cs --library target/release/lwk.dll --out-dir target/release/csharp
    cp target/release/lwk.dll target/release/csharp
    cp lwk_bindings/tests/test_data/test-dotnet.csproj target/release/csharp
    cp lwk_bindings/tests/bindings/list_transactions.cs target/release/csharp


# Run benchmarks. Optionally specify which benchmark to run
bench filter="":
    cd lwk_wollet/benches && cargo bench -- {{filter}} && cd -

# Build the mdbook documentation
mdbook:
    cd docs && mdbook build

# Serve the mdbook documentation locally for development
mdbook-serve: mdbook
    cd docs && mdbook serve
