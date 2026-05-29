#!/bin/bash

SCRIPT_DIR="$(dirname "$0")"
BINARY="$SCRIPT_DIR/launchdock"

"$BINARY" start
"$BINARY" show
