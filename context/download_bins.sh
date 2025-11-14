#!/bin/sh
set -e

mkdir -p bin
cd bin || exit 1

# electrs
FNAME=electrs_linux_esplora_a33e97e1a1fc63fa9c20a116bb92579bbf43b254_liquid && \
curl -Ls https://github.com/RCasatta/electrsd/releases/download/electrs_releases/${FNAME}.gz | gunzip > ${FNAME} && \
chmod +x $FNAME

# waterfalls
FNAME=waterfalls_b8818e1
curl -Ls https://github.com/LeoComandini/waterfalls/releases/download/b8818e1/${FNAME}.gz | gunzip > ${FNAME}
chmod +x $FNAME

# elementsd
curl -Ls https://github.com/ElementsProject/elements/releases/download/elements-23.2.4/elements-23.2.4-x86_64-linux-gnu.tar.gz | tar -xz

# bitcoind
curl -Ls https://bitcoincore.org/bin/bitcoin-core-26.0/bitcoin-26.0-x86_64-linux-gnu.tar.gz | tar -xz


# Binaries for testing kotling bindings
wget https://repo1.maven.org/maven2/net/java/dev/jna/jna/5.13.0/jna-5.13.0.jar

FNAME=kotlin-compiler-1.8.20.zip && wget https://github.com/JetBrains/kotlin/releases/download/v1.8.20/${FNAME} && \
unzip ${FNAME} && rm $FNAME

curl -Ls https://builds.openlogic.com/downloadJDK/openlogic-openjdk/11.0.21+9/openlogic-openjdk-11.0.21+9-linux-x64.tar.gz | tar -xz

# swift
curl -Ls https://download.swift.org/swift-5.5-release/ubuntu1804/swift-5.5-RELEASE/swift-5.5-RELEASE-ubuntu18.04.tar.gz | tar -xz

# wasm
curl https://rustwasm.github.io/wasm-pack/installer/init.sh -sSf | sh
