#!/bin/sh

mkdir -p bin
cd bin || exit 1

# electrs
wget https://github.com/RCasatta/electrsd/releases/download/electrs_releases/electrs_linux_esplora_a33e97e1a1fc63fa9c20a116bb92579bbf43b254_liquid.gz
gunzip electrs_linux_esplora_a33e97e1a1fc63fa9c20a116bb92579bbf43b254_liquid.gz
chmod +x electrs_linux_esplora_a33e97e1a1fc63fa9c20a116bb92579bbf43b254_liquid

# elementsd
elements-22.1.1
wget https://github.com/ElementsProject/elements/releases/download/elements-22.1.1/elements-22.1.1-x86_64-linux-gnu.tar.gz
tar -xzf elements-22.1.1-x86_64-linux-gnu.tar.gz
