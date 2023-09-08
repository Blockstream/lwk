mkdir -p bin
cd bin

# electrs
wget https://github.com/RCasatta/electrsd/releases/download/electrs_releases/electrs_linux_esplora_a33e97e1a1fc63fa9c20a116bb92579bbf43b254_liquid.gz
gunzip electrs_linux_esplora_a33e97e1a1fc63fa9c20a116bb92579bbf43b254_liquid.gz
chmod +x electrs_linux_esplora_a33e97e1a1fc63fa9c20a116bb92579bbf43b254_liquid

# elementsd
wget https://github.com/ElementsProject/elements/releases/download/elements-0.18.1.12/elements-0.18.1.12-x86_64-linux-gnu.tar.gz
tar -xzf elements-0.18.1.12-x86_64-linux-gnu.tar.gz
