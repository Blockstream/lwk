#!/usr/bin/env bash

# Exit on error
set -e

# Default addresses and executables if not specified in environment
LISTEN_ADDR="${LISTEN_ADDR:-127.0.0.1:3000}"
ELEMENTS_ADDR="${ELEMENTS_ADDR:-127.0.0.1:7041}"
ASSET_REGISTRY_ADDR="${ASSET_REGISTRY_ADDR:-127.0.0.1:3023}"
ELEMENTSD_EXEC="${ELEMENTSD_EXEC:-elementsd}"
# Compute ELEMENTS_CLI_EXEC based on ELEMENTSD_EXEC location if not provided
if [ -z "$ELEMENTS_CLI_EXEC" ]; then
    if [[ "$ELEMENTSD_EXEC" == *"/"* ]]; then
        # If ELEMENTSD_EXEC contains a path, look for elements-cli in the same directory
        ELEMENTSD_DIR=$(dirname "$ELEMENTSD_EXEC")
        ELEMENTS_CLI_EXEC="${ELEMENTSD_DIR}/elements-cli"
    else
        # If ELEMENTSD_EXEC is just a command (likely in PATH), assume elements-cli is also in PATH
        ELEMENTS_CLI_EXEC="elements-cli"
    fi
fi
WATERFALLS_EXEC="${WATERFALLS_EXEC:-waterfalls}"
REGISTRY_EXEC="${REGISTRY_EXEC:-server}"

# Create temporary root directory
ROOT_DIR=$(mktemp -d)
echo "Using temporary directory: $ROOT_DIR"

# Create necessary directories
ELEMENTS_DIR="$ROOT_DIR/elements_data"
WATERFALLS_DB="$ROOT_DIR/waterfalls_db"
ASSET_REGISTRY_DB="$ROOT_DIR/asset_registry_db"
mkdir -p "$ELEMENTS_DIR" "$WATERFALLS_DB" "$ASSET_REGISTRY_DB"

# Extract host and port from ELEMENTS_ADDR
ELEMENTS_HOST=$(echo $ELEMENTS_ADDR | cut -d: -f1)
ELEMENTS_PORT=$(echo $ELEMENTS_ADDR | cut -d: -f2)

# Initialize elements-cli command with common arguments
ELEMENTS_CLI_CMD="$ELEMENTS_CLI_EXEC -rpcconnect=$ELEMENTS_HOST -rpcport=$ELEMENTS_PORT -rpcuser=user -rpcpassword=pass"

# Start elementsd
$ELEMENTSD_EXEC \
    -fallbackfee=0.0001 \
    -dustrelayfee=0.00000001 \
    -chain=liquidregtest \
    -initialfreecoins=2100000000 \
    -acceptdiscountct=1 \
    -validatepegin=0 \
    -datadir="$ELEMENTS_DIR" \
    -rest=1 \
    -rpcuser=user \
    -rpcpassword=pass \
    -rpcbind=$ELEMENTS_HOST \
    -rpcport=$ELEMENTS_PORT \
    -txindex=1 \
    -daemon

echo "Waiting for elementsd to start..."
sleep 3

$ELEMENTS_CLI_CMD createwallet test_wallet
$ELEMENTS_CLI_CMD rescanblockchain

# First wpkh_slip77 address of "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about"
$ELEMENTS_CLI_CMD sendtoaddress el1qq2xvpcvfup5j8zscjq05u2wxxjcyewk7979f3mmz5l7uw5pqmx6xf5xy50hsn6vhkm5euwt72x878eq6zxx2z0z676mna6kdq 1

# first wpkh_slip77 address of ledger mnemonic "glory promote mansion idle axis finger extra february uncover one trip resource lawn turtle enact monster seven myth punch hobby comfort wild raise skin"
$ELEMENTS_CLI_CMD sendtoaddress el1qqvk6gl0lgs80w8rargdqyfsl7f0llsttzsx8gd4fz262cjnt0uxh6y68aq4qx76ahvuvlrz8t8ey9v04clsf58w045gzmxga3 1

# Start block generation loop in background
(
    while true; do
        $ELEMENTS_CLI_CMD generatetoaddress 1 $($ELEMENTS_CLI_CMD getnewaddress) | jq -c
        sleep 5
    done
) &

GENERATE_PID=$!

# Start waterfalls in the background
$WATERFALLS_EXEC \
    --network elements-regtest \
    --add-cors \
    --node-url="http://$ELEMENTS_ADDR" \
    --listen="$LISTEN_ADDR" \
    --db-dir="$WATERFALLS_DB" \
    --rpc-user-password="user:pass" &

WATERFALLS_PID=$!

# Start asset registry in the background
SKIP_VERIFY_DOMAIN_LINK=1 $REGISTRY_EXEC \
    --addr "$ASSET_REGISTRY_ADDR" \
    --db-path "$ASSET_REGISTRY_DB" \
    --esplora-url "http://$LISTEN_ADDR" &

ASSET_REGISTRY_PID=$!

POLICY_ASSET=$($ELEMENTS_CLI_CMD getsidechaininfo | jq .pegged_asset)

echo "Using executables:"
echo "  elementsd: $ELEMENTSD_EXEC"
echo "  elements-cli: $ELEMENTS_CLI_EXEC"
echo "  waterfalls: $WATERFALLS_EXEC"
echo "  registry: $REGISTRY_EXEC"
echo
echo "Waterfalls started with address: http://$LISTEN_ADDR"
echo "Elements RPC address: http://$ELEMENTS_ADDR"
echo "Asset Registry address: http://$ASSET_REGISTRY_ADDR"
echo "Policy asset: $POLICY_ASSET"

echo "Press Ctrl+C to stop all services"

# Handle cleanup on script termination
cleanup() {
    echo "Stopping services..."
    kill $WATERFALLS_PID || true
    kill $GENERATE_PID || true
    kill $ASSET_REGISTRY_PID || true
    $ELEMENTS_CLI_CMD stop || true
    echo "Removing temporary directory..."
    rm -rf "$ROOT_DIR"
}

trap cleanup EXIT

# Wait for Ctrl+C
wait $WATERFALLS_PID
