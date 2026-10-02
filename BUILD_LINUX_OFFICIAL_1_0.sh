#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")"
echo "=== Eureka Nexus Miner Official 1.0 ==="
cargo build --release
ls -lh target/release/eureka-nexus-miner-official
