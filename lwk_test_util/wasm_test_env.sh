#!/usr/bin/env bash

# Exit on error
set -e

# Default addresses and executables if not specified in environment
LISTEN_ADDR="${LISTEN_ADDR:-127.0.0.1:3000}"
ELEMENTS_ADDR="${ELEMENTS_ADDR:-127.0.0.1:7041}"
ASSET_REGISTRY_ADDR="${ASSET_REGISTRY_ADDR:-127.0.0.1:3023}"
ELECTRS_HTTP_ADDR="${ELECTRS_HTTP_ADDR:-127.0.0.1:3002}" # required for asset registry server
NEXUS_RELAY_PORT="${NEXUS_RELAY_PORT:-3330}"
JADE_WEBSOCKET_PORT="${JADE_WEBSOCKET_PORT:-3331}"
EMULATOR_PORT="${EMULATOR_PORT:-30121}"
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
ELECTRS_LIQUID_EXEC="${ELECTRS_LIQUID_EXEC:-electrs}"
NEXUS_RELAY_EXEC="${NEXUS_RELAY_EXEC:-nexus_relay}"
WEBSOCAT_EXEC="${WEBSOCAT_EXEC:-websocat}"

# Create temporary root directory
ROOT_DIR=$(mktemp -d)
echo "Using temporary directory: $ROOT_DIR"

# Create necessary directories
ELEMENTS_DIR="$ROOT_DIR/elements_data"
WATERFALLS_DB="$ROOT_DIR/waterfalls_db"
ASSET_REGISTRY_DB="$ROOT_DIR/asset_registry_db"
ELECTRS_DB="$ROOT_DIR/electrs_db"
mkdir -p "$ELEMENTS_DIR" "$WATERFALLS_DB" "$ASSET_REGISTRY_DB" "$ELECTRS_DB"

# Extract host and port from ELEMENTS_ADDR
ELEMENTS_HOST=$(echo $ELEMENTS_ADDR | cut -d: -f1)
ELEMENTS_PORT=$(echo $ELEMENTS_ADDR | cut -d: -f2)
ZMQ_ENDPOINT="tcp://${ELEMENTS_HOST}:29000"

# Initialize elements-cli command with common arguments
ELEMENTS_CLI_CMD="$ELEMENTS_CLI_EXEC -rpcconnect=$ELEMENTS_HOST -rpcport=$ELEMENTS_PORT -rpcuser=user -rpcpassword=pass"

# Start Jade emulator docker container
echo "Starting Jade emulator..."
JADE_CONTAINER_ID=$(docker run -d --rm -p $EMULATOR_PORT:$EMULATOR_PORT xenoky/local-jade-emulator:1.0.27)
echo "Jade emulator container ID: $JADE_CONTAINER_ID"

# Wait for Jade emulator to be ready
echo "Waiting for Jade emulator to start..."
sleep 3

# Start websocat to bridge TCP to WebSocket for Jade
echo "Starting WebSocket bridge for Jade..."
$WEBSOCAT_EXEC --binary ws-listen:127.0.0.1:$JADE_WEBSOCKET_PORT tcp:127.0.0.1:$EMULATOR_PORT &
WEBSOCAT_PID=$!

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
    -zmqpubrawtx=$ZMQ_ENDPOINT \
    -daemon

echo "Waiting for elementsd to start..."
sleep 1

$ELEMENTS_CLI_CMD createwallet test_wallet
$ELEMENTS_CLI_CMD rescanblockchain

# First wpkh_slip77 address of "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about"
$ELEMENTS_CLI_CMD sendtoaddress el1qq2xvpcvfup5j8zscjq05u2wxxjcyewk7979f3mmz5l7uw5pqmx6xf5xy50hsn6vhkm5euwt72x878eq6zxx2z0z676mna6kdq 1

# first wpkh_slip77 address of ledger mnemonic "glory promote mansion idle axis finger extra february uncover one trip resource lawn turtle enact monster seven myth punch hobby comfort wild raise skin"
$ELEMENTS_CLI_CMD sendtoaddress el1qqvk6gl0lgs80w8rargdqyfsl7f0llsttzsx8gd4fz262cjnt0uxh6y68aq4qx76ahvuvlrz8t8ey9v04clsf58w045gzmxga3 1

# Send to jade emulator first address
$ELEMENTS_CLI_CMD sendtoaddress el1qqv7rvrvjzhwzy0v0xpd4lguzwdknf6az2sgvplcvnkedgyn6q25h62k2zskmqgn5x0dpu3xvey2tpnm2mr8hywnnpajzxvrnq 1

# Send to multisig wallet between jade emulator and abandon wallet
# first address of ct(slip77(74819a3e39ffccee0218f9f2164998e01c8fb0797017d62800761f466dd84b51),elwsh(multi(2,[73c5da0a/87'/1'/0']tpubDCChhoz5Qdrkn7Z7KXawq6Ad6r3A4MUkCoVTqeWxfTkA6bHNJ3CHUEtALQdkNeixNz4446PcAmw4WKcj3mV2vb29H7sg9EPzbyCU1y2merw/<0;1>/*,[e3ebcc79/87'/1'/0']tpubDDJ2wnPWhEeV4yxmgoe1YdjxffXP2QTuoVQ1wCGgyFyxZLLKbzXVijZoAXbhkNVJoMVp2UKW1V5NXxdYgENwvx2T4652P4wTxLM1ycTppcu/<0;1>/*)))#wyx8q05s
$ELEMENTS_CLI_CMD sendtoaddress el1qqvfzwj9gep2l6cw0y2lwe94ks2lgv02egrmwyngahd7aseu9yet95fzen9v5harcjn8ug7kdutv3fwndugvdj6tjpz0ajgkmctyjt7577fvwelnvgntk 1

# Start block generation loop in background
(
    while true; do
        $ELEMENTS_CLI_CMD generatetoaddress 1 $($ELEMENTS_CLI_CMD getnewaddress) | jq -c
        sleep 2
    done
) &

GENERATE_PID=$!

# Start electrs in the background
echo "Starting electrs..."
$ELECTRS_LIQUID_EXEC \
    --network liquidregtest \
    --jsonrpc-import \
    --db-dir="$ELECTRS_DB" \
    --daemon-rpc-addr="$ELEMENTS_ADDR" \
    --cookie="user:pass" \
    --http-addr="$ELECTRS_HTTP_ADDR" &

ELECTRS_PID=$!

# Start waterfalls in the background
echo "Starting waterfalls..."
$WATERFALLS_EXEC \
    --network elements-regtest \
    --add-cors \
    --node-url="http://$ELEMENTS_ADDR" \
    --listen="$LISTEN_ADDR" \
    --db-dir="$WATERFALLS_DB" \
    --rpc-user-password="user:pass" &

WATERFALLS_PID=$!

# Start asset registry in the background
# Note: using electrs HTTP endpoint for the esplora URL because it has the /asset endpoint
echo "Starting asset registry..."
SKIP_VERIFY_DOMAIN_LINK=1 $REGISTRY_EXEC \
    --addr "$ASSET_REGISTRY_ADDR" \
    --add-cors \
    --db-path "$ASSET_REGISTRY_DB" \
    --esplora-url "http://$ELECTRS_HTTP_ADDR" &

ASSET_REGISTRY_PID=$!

# Start nexus_relay in the background
echo "Starting nexus_relay..."
$NEXUS_RELAY_EXEC \
    --port $NEXUS_RELAY_PORT \
    --base-url "http://$ELEMENTS_ADDR" \
    --zmq-endpoint "$ZMQ_ENDPOINT" \
    --network elements-regtest &

NEXUS_RELAY_PID=$!

POLICY_ASSET=$($ELEMENTS_CLI_CMD getsidechaininfo | jq .pegged_asset)

echo "Using executables:"
echo "  elementsd: $ELEMENTSD_EXEC"
echo "  elements-cli: $ELEMENTS_CLI_EXEC"
echo "  electrs: $ELECTRS_LIQUID_EXEC"
echo "  waterfalls: $WATERFALLS_EXEC"
echo "  registry: $REGISTRY_EXEC"
echo "  nexus_relay: $NEXUS_RELAY_EXEC"
echo "  websocat: $WEBSOCAT_EXEC"
echo
echo "Waterfalls HTTP API: http://$LISTEN_ADDR"
echo "Elements RPC address: http://$ELEMENTS_ADDR"
echo "Electrs RPC address: $ELECTRS_RPC_ADDR"
echo "Electrs HTTP API: http://$ELECTRS_HTTP_ADDR"
echo "Asset Registry address: http://$ASSET_REGISTRY_ADDR"
echo "Nexus Relay WebSocket: ws://localhost:$NEXUS_RELAY_PORT"
echo "Jade WebSocket Bridge: ws://localhost:$JADE_WEBSOCKET_PORT"
echo "Policy asset: $POLICY_ASSET"

echo "Press Ctrl+C to stop all services"

# Need to set these env vars for tests
export ELECTRS_LIQUID_EXEC
export ELEMENTSD_EXEC

# Handle cleanup on script termination
cleanup() {
    echo "Stopping services..."
    kill $WATERFALLS_PID || true
    kill $GENERATE_PID || true
    kill $ASSET_REGISTRY_PID || true
    kill $ELECTRS_PID || true
    kill $NEXUS_RELAY_PID || true
    kill $WEBSOCAT_PID || true
    $ELEMENTS_CLI_CMD stop || true
    if [ ! -z "$JADE_CONTAINER_ID" ]; then
        echo "Stopping Jade emulator container..."
        docker stop $JADE_CONTAINER_ID || true
    fi
    echo "Removing temporary directory..."
    rm -rf "$ROOT_DIR"
}

trap cleanup EXIT

# Wait for Ctrl+C
wait $WATERFALLS_PID
