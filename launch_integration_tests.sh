#!/bin/bash

# launch with DEBUG env equal something when debugging to see outputs

# use externally defined env vars or the following defaults
export BITCOIND_EXEC=${BITCOIND_EXEC:=$PWD/server/bitcoin-0.20.1/bin/bitcoind}
export ELECTRS_EXEC=${ELECTRS_EXEC:=$PWD/server/electrs_bitcoin/bin/electrs}

export ELEMENTSD_EXEC=${ELEMENTSD_EXEC:=$PWD/server/elements-0.18.1.8/bin/elementsd}
export ELECTRS_LIQUID_EXEC=${ELECTRS_LIQUID_EXEC:=$PWD/server/electrs_liquid/bin/electrs}

if [[ -z "${DEBUG}" ]]; then
  NOCAPTURE=""
else
  NOCAPTURE="-- --nocapture"
fi

# delete any previoulsy launched integation test process
ps -eaf | grep -v grep | grep electrum_integration_test | awk '{print $2}' | xargs -r kill -9

# launch tests, use liquid or bitcoin as parameter to launch only the respective
RUST_BACKTRACE=1 cargo test $1 $NOCAPTURE
