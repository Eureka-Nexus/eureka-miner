#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")"
cargo build --release --locked --bin eureka-nexus-miner-official
python3 tools/package-linux.py
