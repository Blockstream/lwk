#!/bin/sh
set -e

# To calculate SHA256 for a new binary: curl -sL <URL> | sha256sum

mkdir -p bin
cd bin || exit 1

# electrs
ELECTRS_FILENAME="electrs_linux_esplora_027e38d3ebc2f85b28ae76f8f3448438ee4fc7b1_liquid.zip"
ELECTRS_EXPECTED_SHA256="a63a314c16bc6642fc060bbc19bd1d54ebf86b42188ff2a11c705177c1eb22f7"

wget "https://github.com/RCasatta/electrsd/releases/download/electrs_releases/${ELECTRS_FILENAME}"
echo "${ELECTRS_EXPECTED_SHA256}  ${ELECTRS_FILENAME}" | sha256sum -c -
unzip "${ELECTRS_FILENAME}" && rm "${ELECTRS_FILENAME}"
ELECTRS_NAME="${ELECTRS_FILENAME%.zip}"
mv electrs "${ELECTRS_NAME}" && chmod +x "${ELECTRS_NAME}"

# waterfalls
WATERFALLS_FILENAME="waterfalls_b8818e1.gz"
WATERFALLS_EXPECTED_SHA256="6e851ce656cf4ff6ff7dca9c2e5565c3d7921376a9f5c23b1f9aa746fee667fb"

curl -Ls "https://github.com/LeoComandini/waterfalls/releases/download/b8818e1/${WATERFALLS_FILENAME}" -o "${WATERFALLS_FILENAME}"
echo "${WATERFALLS_EXPECTED_SHA256}  ${WATERFALLS_FILENAME}" | sha256sum -c -
gunzip "${WATERFALLS_FILENAME}"
WATERFALLS_NAME="${WATERFALLS_FILENAME%.gz}"
chmod +x "${WATERFALLS_NAME}"

# elementsd
ELEMENTSD_VERSION=23.3.1
ELEMENTSD_EXPECTED_SHA256="864e3a8240137c4e948ecae7c526ccb363771351ea68737a14c682025d5fedaa"

ELEMENTSD_FILENAME="elements-${ELEMENTSD_VERSION}-x86_64-linux-gnu.tar.gz"
curl -Ls "https://github.com/ElementsProject/elements/releases/download/elements-${ELEMENTSD_VERSION}/${ELEMENTSD_FILENAME}" -o "${ELEMENTSD_FILENAME}"
echo "${ELEMENTSD_EXPECTED_SHA256}  ${ELEMENTSD_FILENAME}" | sha256sum -c -
tar -xzf "${ELEMENTSD_FILENAME}" && rm "${ELEMENTSD_FILENAME}"

# bitcoind
BITCOIND_VERSION=26.0
BITCOIND_EXPECTED_SHA256="23e5ab226d9e01ffaadef5ffabe8868d0db23db952b90b0593652993680bb8ab"

BITCOIND_FILENAME="bitcoin-${BITCOIND_VERSION}-x86_64-linux-gnu.tar.gz"
curl -Ls "https://bitcoincore.org/bin/bitcoin-core-${BITCOIND_VERSION}/${BITCOIND_FILENAME}" -o "${BITCOIND_FILENAME}"
echo "${BITCOIND_EXPECTED_SHA256}  ${BITCOIND_FILENAME}" | sha256sum -c -
tar -xzf "${BITCOIND_FILENAME}" && rm "${BITCOIND_FILENAME}"

# asset registry
ASSET_REGISTRY_FILENAME="asset_registry_server_5ecf533.gz"
ASSET_REGISTRY_EXPECTED_SHA256="fbfbb996954d6e369f3e5202ef3e2d4885f1fc34b49a2af63be5278e267d9d62"

curl -Ls "https://github.com/LeoComandini/asset_registry/releases/download/5ecf533/${ASSET_REGISTRY_FILENAME}" -o "${ASSET_REGISTRY_FILENAME}"
echo "${ASSET_REGISTRY_EXPECTED_SHA256}  ${ASSET_REGISTRY_FILENAME}" | sha256sum -c -
gunzip "${ASSET_REGISTRY_FILENAME}"
ASSET_REGISTRY_NAME="${ASSET_REGISTRY_FILENAME%.gz}"
chmod +x "${ASSET_REGISTRY_NAME}"

# Binaries for testing kotlin bindings

# jna (SHA1 from Maven: 1200e7ebeedbe0d10062093f32925a912020e747)
JNA_VERSION=5.13.0
JNA_EXPECTED_SHA256="66d4f819a062a51a1d5627bffc23fac55d1677f0e0a1feba144aabdd670a64bb"

JNA_FILENAME="jna-${JNA_VERSION}.jar"
wget "https://repo1.maven.org/maven2/net/java/dev/jna/jna/${JNA_VERSION}/${JNA_FILENAME}"
echo "${JNA_EXPECTED_SHA256}  ${JNA_FILENAME}" | sha256sum -c -

# kotlin
KOTLIN_VERSION=1.8.20
KOTLIN_EXPECTED_SHA256="10df74c3c6e2eafd4c7a5572352d37cbe41774996e42de627023cb4c82b50ae4"

KOTLIN_FILENAME="kotlin-compiler-${KOTLIN_VERSION}.zip"
wget "https://github.com/JetBrains/kotlin/releases/download/v${KOTLIN_VERSION}/${KOTLIN_FILENAME}"
echo "${KOTLIN_EXPECTED_SHA256}  ${KOTLIN_FILENAME}" | sha256sum -c -
unzip "${KOTLIN_FILENAME}" && rm "${KOTLIN_FILENAME}"

# openlogic-openjdk
OPENJDK_VERSION="11.0.21+9"
OPENJDK_EXPECTED_SHA256="a6ceb8a550e63a7592d9c990ae70f998b96cc4fd6d141b39c10e21b92bfb4fca"

OPENJDK_FILENAME="openlogic-openjdk-${OPENJDK_VERSION}-linux-x64.tar.gz"
curl -Ls "https://builds.openlogic.com/downloadJDK/openlogic-openjdk/${OPENJDK_VERSION}/${OPENJDK_FILENAME}" -o "${OPENJDK_FILENAME}"
echo "${OPENJDK_EXPECTED_SHA256}  ${OPENJDK_FILENAME}" | sha256sum -c -
tar -xzf "${OPENJDK_FILENAME}" && rm "${OPENJDK_FILENAME}"

# swift
SWIFT_VERSION="5.5-RELEASE"
SWIFT_EXPECTED_SHA256="1ebf6441938dafc9fba85419b0482f4b6d371e0d2d1851e80ae6769b11aab6a5"

SWIFT_FILENAME="swift-${SWIFT_VERSION}-ubuntu18.04.tar.gz"
curl -Ls "https://download.swift.org/swift-5.5-release/ubuntu1804/swift-${SWIFT_VERSION}/${SWIFT_FILENAME}" -o "${SWIFT_FILENAME}"
echo "${SWIFT_EXPECTED_SHA256}  ${SWIFT_FILENAME}" | sha256sum -c -
tar -xzf "${SWIFT_FILENAME}" && rm "${SWIFT_FILENAME}"

# wasm-pack
WASMPACK_VERSION="0.13.1"
WASMPACK_EXPECTED_SHA256="c539d91ccab2591a7e975bcf82c82e1911b03335c80aa83d67ad25ed2ad06539"

WASMPACK_FILENAME="wasm-pack-v${WASMPACK_VERSION}-x86_64-unknown-linux-musl.tar.gz"
curl -Ls "https://github.com/rustwasm/wasm-pack/releases/download/v${WASMPACK_VERSION}/${WASMPACK_FILENAME}" -o "${WASMPACK_FILENAME}"
echo "${WASMPACK_EXPECTED_SHA256}  ${WASMPACK_FILENAME}" | sha256sum -c -
tar -xzf "${WASMPACK_FILENAME}" --strip-components=1 -C /usr/local/bin "wasm-pack-v${WASMPACK_VERSION}-x86_64-unknown-linux-musl/wasm-pack"
rm "${WASMPACK_FILENAME}"
