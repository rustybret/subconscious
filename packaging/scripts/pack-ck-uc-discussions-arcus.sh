#!/bin/sh
# Stage the release binary and hand it to the Arcus toolchain's own pack-arcus.sh (schema-3 envelope,
# canonical release_id, no hand-written submission.json). Then `publish-arcus.sh` emits the bundle.
# Usage: pack-ck-uc-discussions-arcus.sh --sequence N [--version V]   (cargo build --release first)
set -eu
SCRIPT_DIR="$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)"
REPO_ROOT="$(CDPATH= cd -- "${SCRIPT_DIR}/../.." && pwd)"
CONFIG="${REPO_ROOT}/packaging/arcus/ck-uc-discussions.json"
SEQUENCE=""; VERSION=""
while [ "$#" -gt 0 ]; do case "$1" in
  --sequence) SEQUENCE="$2"; shift 2;; --version) VERSION="$2"; shift 2;;
  *) printf 'unknown option: %s\n' "$1" >&2; exit 2;; esac; done
[ -n "$SEQUENCE" ] || { echo "--sequence is required: arcus manifest allocate-sequence --gateway https://arcus-auth.rustybret.com --package-id ck-uc-discussions" >&2; exit 2; }
[ -n "$VERSION" ] || VERSION="$(sed -n 's/.*"version"[[:space:]]*:[[:space:]]*"\([^"]*\)".*/\1/p' "$CONFIG" | head -n 1)"
TARGET_DIR="${CARGO_TARGET_DIR:-$(cd "$REPO_ROOT" && cargo metadata --format-version 1 --no-deps | sed -n 's/.*"target_directory":"\([^"]*\)".*/\1/p')}"
BIN="${TARGET_DIR}/release/ck-uc-discussions"
[ -x "$BIN" ] || { echo "missing $BIN: run cargo build --release --locked -p uc-discussions first" >&2; exit 1; }
STAGE="$(mktemp -d "${TMPDIR:-/tmp}/arcus-ck-uc-discussions.XXXXXX")"; trap 'rm -rf "$STAGE"' EXIT INT TERM
cp "$BIN" "$STAGE/"
cd "$REPO_ROOT"
sh packaging/scripts/pack-arcus.sh --config "$CONFIG" --package-id ck-uc-discussions --version "$VERSION" \
  --sequence "$SEQUENCE" --offline --payload "$STAGE" --target darwin-arm64 --action-executable ck-uc-discussions \
  --key-file "${ARCUS_KEY_FILE:-$HOME/.config/arcus/signing.key}" --output "${REPO_ROOT}/dist-arcus"
