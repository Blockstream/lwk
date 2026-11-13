# list available recipes
default:
    just --list

SIMPLICITY_FEATURES := if env_var_or_default("SIMPLICITY", "") != "" { "--features simplicity" } else { "" }
LIB_EXTENSION := if os() == "macos" { "dylib" } else { "so" }

# No gobley release supports UniFFI 0.31 yet. This revision is the UniFFI 0.31.2 port proposed
# upstream in https://github.com/gobley/gobley/pull/289, plus the fix for u8/u16 return values
# misread on 32-bit ARM Android (https://github.com/gobley/gobley/issues/252), which otherwise
# fails the UniFFI checksum check once the app is AOT-compiled.
GOBLEY_GIT := "https://github.com/angelix/gobley"
GOBLEY_REV := "26632c5948d0f42865c03d94df9e07729cea6cfa"
GOBLEY_ROOT := "target/gobley"

# Third-party UniFFI generators must match the UniFFI version used by lwk_bindings.
# Recipes reinstall them when the installed version differs, so a stale binary on PATH is not reused.
UNIFFI_BINDGEN_GO_TAG := "v0.7.1+v0.31.0"
UNIFFI_BINDGEN_CS_TAG := "v0.11.1+v0.31.0"

# cargo-zigbuild and cargo-xwin cross build the Linux and Windows libs bundled in the KMP jvm jar
CARGO_ZIGBUILD_VERSION := "0.24.0"
CARGO_XWIN_VERSION := "0.23.1"

# Versions of the MSVC CRT and Windows SDK that cargo-xwin downloads. It reuses its cache whatever the
# versions in it, so each pair of versions gets its own cache directory.
export XWIN_CRT_VERSION := "14.44.17.14"
export XWIN_SDK_VERSION := "10.0.26100"
export XWIN_CACHE_DIR := cache_directory() / "cargo-xwin" / ("crt-" + XWIN_CRT_VERSION + "-sdk-" + XWIN_SDK_VERSION)
CROSS_TOOLS_ROOT := "target/cross-tools"
CROSS_TOOLS_BIN := justfile_directory() / CROSS_TOOLS_ROOT / "bin"

# JNA loads the native lib from the classpath directory named after the platform, for ex. darwin-aarch64
JVM_RESOURCES := "lwk_bindings/android_bindings/lib/src/jvmMain/resources"

# The Windows libs link the C runtime statically, so they load without the Visual C++ Redistributable.
# The flag is appended to RUSTFLAGS, since a set RUSTFLAGS replaces the target rustflags.
WINDOWS_RUSTFLAGS := "-C target-feature=+crt-static"

# build the bindings lib: liblwk.so (as specified in lwk_bindings/Cargo.toml)
build-bindings-lib:
    # a debug build would be fine if used only to generate interfaces files but some jobs use it to package it, thus release is necessary.
    cargo build --release -p lwk_bindings --features test_env {{SIMPLICITY_FEATURES}}

# build liblwk in release from scratch in `target_dir` (deleted first), write the hashes of the artifacts to `out`
build-reproducible out target_dir="target/reproducible":
    rm -rf {{target_dir}}
    cargo build --locked --release -p lwk_bindings --target-dir {{target_dir}}
    mkdir -p {{out}}
    cd {{target_dir}}/release && sha256sum liblwk.so liblwk.a | tee {{absolute_path(out)}}/sha256sums.txt

# compare the artifact hashes of two `build-reproducible` outputs
compare-reproducible a b:
    #!/usr/bin/env bash
    set -euo pipefail
    for f in {{a}}/sha256sums.txt {{b}}/sha256sums.txt; do
        test -f "$f" || { echo "missing $f" >&2; exit 2; }
    done
    if diff {{a}}/sha256sums.txt {{b}}/sha256sums.txt; then
        echo "build is reproducible"
    else
        echo "build is NOT reproducible"
        exit 1
    fi

# build the python interface "lwk.py"
python-build-bindings: build-bindings-lib
    # note production wheels are built with maturin, but this can be useful during development
    # we use release to generate interfaces only because build-bindings-lib is done in release, so we find intermediate packages
    cargo run --release --features bindings -- generate --library target/release/liblwk.{{LIB_EXTENSION}} --language python --out-dir target/release/bindings
    cp target/release/liblwk.{{LIB_EXTENSION}} target/release/bindings

# smoke test the python bindings
python-test-bindings: python-build-bindings
    PYTHONPATH=target/release/bindings/ python3 -c 'import lwk'

# build the bindings lib with simplicity feature
build-bindings-lib-simplicity:
    cargo build --release -p lwk_bindings --features simplicity,test_env

# build the python bindings with simplicity enabled
python-build-bindings-simplicity: build-bindings-lib-simplicity
    cargo run --release --features bindings,simplicity -- generate --library target/release/liblwk.{{LIB_EXTENSION}} --language python --out-dir target/release/bindings
    cp target/release/liblwk.{{LIB_EXTENSION}} target/release/bindings

# smoke test the python bindings with simplicity
python-test-bindings-simplicity: python-build-bindings-simplicity
    PYTHONPATH=target/release/bindings/ python3 lwk_bindings/tests/bindings/simplicity_p2pk.py
    PYTHONPATH=target/release/bindings/ python3 lwk_bindings/tests/bindings/simplicity_p2pk_regtest.py
    PYTHONPATH=target/release/bindings/ python3 lwk_bindings/tests/bindings/simplicity_options_regtest.py

# build the python bindings and start a python env with them
python-env-bindings: python-build-bindings
    PYTHONPATH=target/release/bindings/ python3

# build the docker "xenoky/lwk-builder" used in the CI
docker-build:
    cd context && docker build . -t xenoky/lwk-builder && cd -

# push the docker "xenoky/lwk-builder" on docker hub
docker-push: docker-build
    docker push xenoky/lwk-builder # require credentials

kotlin: build-bindings-lib
    cargo run --release --features bindings -- generate --library target/release/liblwk.{{LIB_EXTENSION}} --language kotlin --out-dir target/release/kotlin
    cp -a target/release/kotlin/lwk lwk_bindings/android_bindings/lib/src/androidMain/kotlin

# Cross build the lib for aarch64-linux-android
aarch64-linux-android:
	cargo ndk -t aarch64-linux-android -o target/release/android/jniLibs build --release -p lwk_bindings {{SIMPLICITY_FEATURES}}

# Cross build the lib for armv7-linux-androideabi
armv7-linux-androideabi:
	cargo ndk -t armv7-linux-androideabi -o target/release/android/jniLibs build --release -p lwk_bindings {{SIMPLICITY_FEATURES}}

# Cross build the lib for i686-linux-android
i686-linux-android:
	cargo ndk -t i686-linux-android -o target/release/android/jniLibs build --release -p lwk_bindings {{SIMPLICITY_FEATURES}}

# Cross build the lib for x86_64-linux-android
x86_64-linux-android:
	cargo ndk -t x86_64-linux-android -o target/release/android/jniLibs build --release -p lwk_bindings {{SIMPLICITY_FEATURES}}

# After cross building all the lib for android put them in final dir
android: aarch64-linux-android armv7-linux-androideabi i686-linux-android x86_64-linux-android
    cp -a target/release/android/jniLibs lwk_bindings/android_bindings/lib/src/androidMain

# Build the kotlin multiplatform interface and android, ios and jvm
kotlin-multiplatform: ios ios-sim android jvm
    cargo install --locked --git {{GOBLEY_GIT}} --rev {{GOBLEY_REV}} --root {{GOBLEY_ROOT}} --bin gobley-uniffi-bindgen gobley-uniffi-bindgen
    {{GOBLEY_ROOT}}/bin/gobley-uniffi-bindgen --config ./lwk_bindings/uniffi.kotlin-multiplatform.toml --library target/aarch64-apple-ios/release/liblwk.a  --out-dir target/release/kotlin-multiplatform
    cp -a target/release/kotlin-multiplatform/* lwk_bindings/android_bindings/lib/src/
    mkdir -p ./lwk_bindings/android_bindings/lib/src/libs/ios-arm64/
    mkdir -p ./lwk_bindings/android_bindings/lib/src/libs/ios-simulator-arm64/
    cp target/aarch64-apple-ios/release/liblwk.a lwk_bindings/android_bindings/lib/src/libs/ios-arm64/
    cp target/lipo-ios-sim/release/liblwk.a lwk_bindings/android_bindings/lib/src/libs/ios-simulator-arm64/

# Build the desktop libs (macOS, Linux and Windows on x86_64 and arm64) and put them in the jvm resources
jvm: aarch64-apple-darwin x86_64-apple-darwin aarch64-unknown-linux-gnu x86_64-unknown-linux-gnu aarch64-pc-windows-msvc x86_64-pc-windows-msvc
    mkdir -p {{JVM_RESOURCES}}/darwin-aarch64 {{JVM_RESOURCES}}/darwin-x86-64 {{JVM_RESOURCES}}/linux-aarch64 {{JVM_RESOURCES}}/linux-x86-64 {{JVM_RESOURCES}}/win32-aarch64 {{JVM_RESOURCES}}/win32-x86-64
    cp -a target/aarch64-apple-darwin/release/liblwk.dylib {{JVM_RESOURCES}}/darwin-aarch64/
    cp -a target/x86_64-apple-darwin/release/liblwk.dylib {{JVM_RESOURCES}}/darwin-x86-64/
    cp -a target/aarch64-unknown-linux-gnu/release/liblwk.so {{JVM_RESOURCES}}/linux-aarch64/
    cp -a target/x86_64-unknown-linux-gnu/release/liblwk.so {{JVM_RESOURCES}}/linux-x86-64/
    cp -a target/aarch64-pc-windows-msvc/release/lwk.dll {{JVM_RESOURCES}}/win32-aarch64/
    cp -a target/x86_64-pc-windows-msvc/release/lwk.dll {{JVM_RESOURCES}}/win32-x86-64/

# Install the cargo subcommands used to cross build the Linux and Windows libs, cargo-zigbuild also requires zig (`brew install zig`)
cross-tools:
    cargo install --locked --root {{CROSS_TOOLS_ROOT}} cargo-zigbuild@{{CARGO_ZIGBUILD_VERSION}} cargo-xwin@{{CARGO_XWIN_VERSION}}

# Build aarch64-apple-darwin
aarch64-apple-darwin: (apple-darwin "aarch64-apple-darwin")

# Build x86_64-apple-darwin
x86_64-apple-darwin: (apple-darwin "x86_64-apple-darwin")

# Build a macOS dylib. The cargo strip runs rust-objcopy, which can leave the LINKEDIT string pool
# mis-aligned, and dyld refuses to load such a dylib: the system strip is used instead. It removes the
# local symbols too, loading the library needs only the exported ones.
[private]
apple-darwin target:
    MACOSX_DEPLOYMENT_TARGET=11.0 CARGO_PROFILE_RELEASE_STRIP=false cargo build --release --target {{target}} -p lwk_bindings {{SIMPLICITY_FEATURES}}
    /usr/bin/strip -x target/{{target}}/release/liblwk.dylib

# Cross build aarch64-unknown-linux-gnu, linking glibc 2.17
aarch64-unknown-linux-gnu: (linux-gnu "aarch64-unknown-linux-gnu")

# Cross build x86_64-unknown-linux-gnu, linking glibc 2.17
x86_64-unknown-linux-gnu: (linux-gnu "x86_64-unknown-linux-gnu")

# Cross build a Linux lib with zig. Optimizing zig cc defines NDEBUG, unlike gcc and clang, and
# simplicity-sys refuses to build without assertions: NDEBUG is undefined to compile as the other compilers.
[private]
linux-gnu target: cross-tools
    PATH="{{CROSS_TOOLS_BIN}}:$PATH" CFLAGS_{{replace(target, "-", "_")}}=-UNDEBUG cargo zigbuild --release --target {{target}}.2.17 -p lwk_bindings {{SIMPLICITY_FEATURES}}

# Cross build x86_64-pc-windows-msvc, the MSVC CRT and Windows SDK are downloaded on first use
x86_64-pc-windows-msvc: cross-tools
    PATH="{{CROSS_TOOLS_BIN}}:$PATH" RUSTFLAGS="${RUSTFLAGS:-} {{WINDOWS_RUSTFLAGS}}" cargo xwin build --release --target x86_64-pc-windows-msvc -p lwk_bindings {{SIMPLICITY_FEATURES}}

# Cross build aarch64-pc-windows-msvc, the MSVC CRT and Windows SDK are downloaded on first use
aarch64-pc-windows-msvc: cross-tools
    #!/usr/bin/env bash
    set -euo pipefail
    # On this target ring requires clang and the psm assembly is preprocessed only when not using clang-cl,
    # thus clang compiles all the C code. The SDK headers are given in INCLUDE instead of the clang-cl /imsvc
    # flags, so clang searches its own headers first and aws-lc gets the clang arm_neon.h.
    export PATH="{{CROSS_TOOLS_BIN}}:$PATH"
    export RUSTFLAGS="${RUSTFLAGS:-} {{WINDOWS_RUSTFLAGS}}"
    # Assigned first, so a failing cargo xwin env stops the script: eval would ignore its exit status
    xwin_env=$(cargo xwin env --target aarch64-pc-windows-msvc)
    eval "$xwin_env"
    # cargo xwin env moves RUSTFLAGS into the target rustflags and exports it empty, cargo would use the empty one
    unset RUSTFLAGS
    INCLUDE=$(echo "$CFLAGS_aarch64_pc_windows_msvc" | grep -o '/imsvc [^ ]*' | cut -d' ' -f2 | paste -sd ';' -)
    export INCLUDE
    export CC_aarch64_pc_windows_msvc=clang CXX_aarch64_pc_windows_msvc=clang
    export CFLAGS_aarch64_pc_windows_msvc= CXXFLAGS_aarch64_pc_windows_msvc=
    cargo build --release --target aarch64-pc-windows-msvc -p lwk_bindings {{SIMPLICITY_FEATURES}}

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
    xcodegen generate --spec lwk_bindings/swift/project.yml
    xcodebuild archive -project lwk_bindings/swift/LwkBindings.xcodeproj -scheme lwkFFI -archivePath "./target/ios.xcarchive" -sdk iphoneos -destination "generic/platform=iOS"
    xcodebuild archive -project lwk_bindings/swift/LwkBindings.xcodeproj -scheme lwkFFI -archivePath "./target/ios_sim.xcarchive" -sdk iphonesimulator -destination "generic/platform=iOS Simulator"
    xcodebuild -create-xcframework -framework "./target/ios.xcarchive/Products/Library/Frameworks/lwkFFI.framework" -framework "./target/ios_sim.xcarchive/Products/Library/Frameworks/lwkFFI.framework" -output "./target/lwkFFI.xcframework"

# build the Go interface "lwk/lwk.go"
go-build-bindings: build-bindings-lib
    cargo install --list | grep -qF "uniffi-bindgen-go {{UNIFFI_BINDGEN_GO_TAG}} " || cargo install --locked --force uniffi-bindgen-go --git https://github.com/NordSecurity/uniffi-bindgen-go --tag {{UNIFFI_BINDGEN_GO_TAG}}
    mkdir -p lwk_bindings/go/lwk
    cp target/release/liblwk.{{LIB_EXTENSION}} lwk_bindings/go/lwk
    uniffi-bindgen-go --library lwk_bindings/go/lwk/liblwk.{{LIB_EXTENSION}} --out-dir lwk_bindings/go

# smoke test the Go bindings
go-test-bindings: go-build-bindings
    cd lwk_bindings/go && \
    for file in *.go; do \
        echo "running $file"; \
        CGO_LDFLAGS="-L./lwk -llwk" LD_LIBRARY_PATH=./lwk go run "$file" || exit 1; \
    done

csharp-windows: build-bindings-lib
    cargo install --list | grep -qF "uniffi-bindgen-cs {{UNIFFI_BINDGEN_CS_TAG}} " || cargo install --locked --force uniffi-bindgen-cs --git https://github.com/NordSecurity/uniffi-bindgen-cs --tag {{UNIFFI_BINDGEN_CS_TAG}}
    uniffi-bindgen-cs --library target/release/lwk.dll --out-dir target/release/csharp
    python scripts/patch_csharp_finalize.py target/release/csharp/lwk.cs
    cp target/release/lwk.dll target/release/csharp
    cp lwk_bindings/tests/test_data/test-dotnet.csproj target/release/csharp
    cp lwk_bindings/tests/bindings/list_transactions.cs target/release/csharp


# Run benchmarks. Optionally specify which benchmark to run
bench filter="":
    cd lwk_wollet/benches && cargo bench -- {{filter}} && cd -

# Build the mdbook documentation
mdbook:
    cd docs && mdbook build
    cd docs && python3 generate_llms.py

# Build LLM-friendly docs artifacts
llms:
    cd docs && python3 generate_llms.py

# Build all documentation and fail on broken local links
docs-check:
    bash docs/check.sh

# Serve the mdbook documentation locally for development
mdbook-serve: docs-check
    cd docs && mdbook serve --port 3001

# Generate a local Cobertura and HTML coverage report with nextest
coverage-local:
    #!/usr/bin/env bash
    set -euo pipefail

    host_triple="$(rustc -vV | sed -n 's/^host: //p')"
    llvm_path="$(rustc --print sysroot)/lib/rustlib/${host_triple}/bin"
    coverage_profraw_dir="$(pwd)/target/coverage-profraw"

    mkdir -p "${coverage_profraw_dir}"
    find "${coverage_profraw_dir}" -name '*.profraw' -delete
    rm -rf target/coverage

    export RUSTFLAGS='-C instrument-coverage'
    export LLVM_PROFILE_FILE="${coverage_profraw_dir}/coverage-%p-%m.profraw"

    cargo nextest run

    grcov "${coverage_profraw_dir}" -s . --binary-path ./target/debug/ --llvm-path "${llvm_path}" -t cobertura,html --branch \
        --ignore-not-existing --ignore "*cargo*" \
        --ignore "lwk_boltz/regtest/data/**" \
        -o ./target/coverage/ -p "$(pwd)"

    coverage_index="$(pwd)/target/coverage/index.html"
    printf '\nCoverage HTML report: file://%s\n' "${coverage_index}"
