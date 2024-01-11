#!/bin/sh
export ELECTRS_LIQUID_EXEC="$PWD/bin/electrs_linux_esplora_a33e97e1a1fc63fa9c20a116bb92579bbf43b254_liquid"
export ELEMENTSD_EXEC="$PWD/bin/elements-22.1.1/bin/elementsd"
export JADE_EMULATOR_IMAGE_NAME=xenoky/local-jade-emulator
export JADE_EMULATOR_IMAGE_VERSION="1.0.23"
export PIN_SERVER_IMAGE_NAME=tulipan81/blind_pin_server
export PIN_SERVER_IMAGE_VERSION=v0.0.3
export ANDROID_NDK_HOME="$PWD/bin/android-ndk"
export LIB_EXT=$([ $(uname) == "Darwin" ] && echo "dylib" || echo "so")
