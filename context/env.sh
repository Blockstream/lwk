#!/bin/sh
export ELECTRS_LIQUID_EXEC="$PWD/bin/electrs_linux_esplora_a33e97e1a1fc63fa9c20a116bb92579bbf43b254_liquid"
export ELEMENTSD_EXEC="$PWD/bin/elements-23.2.4/bin/elementsd"
export BITCOIND_EXEC="$PWD/bin/bitcoin-26.0/bin/bitcoind"
export WATERFALLS_EXEC="$PWD/bin/waterfalls_b8818e1"
export ASSET_REGISTRY_EXEC="$PWD/bin/asset_registry_server_5ecf533"
export JADE_EMULATOR_IMAGE_NAME=xenoky/local-jade-emulator
export JADE_EMULATOR_IMAGE_VERSION="1.0.27"
export PIN_SERVER_IMAGE_NAME=tulipan81/blind_pin_server
export PIN_SERVER_IMAGE_VERSION=v0.0.7
export ANDROID_NDK_HOME="$PWD/bin/android-ndk"
export LIB_EXT=$([ $(uname) == "Darwin" ] && echo "dylib" || echo "so")
export CLASSPATH="$CLASSPATH:$PWD/bin/jna-5.13.0.jar"
export PATH="$PATH:$PWD/bin/kotlinc/bin:$PWD/bin/openlogic-openjdk-11.0.21+9-linux-x64/bin:$PWD/bin/swift-5.5-RELEASE-ubuntu18.04/usr/bin"
export WASM_BINDGEN_TEST_TIMEOUT=60
