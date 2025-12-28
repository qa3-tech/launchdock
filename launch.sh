#!/bin/bash

SCRIPT_DIR="$(dirname "$0")"
BINARY="$SCRIPT_DIR/launchdock"

if ! "$BINARY" status | grep -q "Daemon: running"; then
    "$BINARY" start
    sleep 0.1
fi

"$BINARY" show