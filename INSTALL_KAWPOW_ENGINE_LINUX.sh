#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")"

VERSION="1.2.4"
ARCHIVE="kawpowminer-ubuntu20-cuda11-${VERSION}.tar.gz"
CHECKSUM="${ARCHIVE}.sha256sum"
BASE="https://github.com/RavenCommunity/kawpowminer/releases/download/${VERSION}"
DEST="engines/kawpow"
TMP="${DEST}/.download"

mkdir -p "$TMP" "$DEST"

echo "Downloading upstream RavenCommunity kawpowminer ${VERSION} (CUDA 11 build)..."
curl -fL --retry 3 --retry-delay 2 "$BASE/$ARCHIVE" -o "$TMP/$ARCHIVE"
curl -fL --retry 3 --retry-delay 2 "$BASE/$CHECKSUM" -o "$TMP/$CHECKSUM"

echo "Verifying upstream SHA-256..."
(
  cd "$TMP"
  sha256sum -c "$CHECKSUM"
)

rm -rf "$TMP/extracted"
mkdir -p "$TMP/extracted"
tar -xzf "$TMP/$ARCHIVE" -C "$TMP/extracted"

ENGINE="$(find "$TMP/extracted" -type f -name kawpowminer | head -1 || true)"
if [[ -z "$ENGINE" ]]; then
  echo "kawpowminer executable not found in upstream archive"
  exit 1
fi

cp "$ENGINE" "$DEST/kawpowminer"
chmod +x "$DEST/kawpowminer"

cat > "$DEST/NOTICE.txt" <<'NOTICE'
Third-party GPU engine: RavenCommunity/kawpowminer 1.2.4
Source: https://github.com/RavenCommunity/kawpowminer
Release: https://github.com/RavenCommunity/kawpowminer/releases/tag/1.2.4
License: GNU General Public License v3.0 (GPL-3.0)

This executable is a separate upstream program launched as a child process by
Eureka Nexus Miner Official 1.0. It is not linked into the Eureka Nexus Rust
binary. The upstream archive and checksum are downloaded directly from the
RavenCommunity GitHub release.
NOTICE

echo
echo "Installed: $DEST/kawpowminer"
"$DEST/kawpowminer" --version || true
