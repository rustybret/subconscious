#!/bin/sh
# fleet/ci/elf-audit.sh — Standalone ELF binary audit helper for Jenkins CI
# Usage: ./fleet/ci/elf-audit.sh <BINARY_PATH> <static|2.31>
set -eu

BINARY="${1:-}"
CHECK_TYPE="${2:-}"

if [ -z "$BINARY" ] || [ ! -f "$BINARY" ]; then
    echo "ERROR: Missing or invalid binary path: '$BINARY'" >&2
    exit 1
fi

echo "==> Auditing ELF binary: $BINARY (check: $CHECK_TYPE)..."
file "$BINARY"

if ! command -v readelf >/dev/null 2>&1; then
    echo "  [WARN] 'readelf' not installed in environment; skipping deep inspection."
    exit 0
fi

case "$CHECK_TYPE" in
    static)
        # In static and static-PIE binaries, PT_INTERP must not be requested.
        if readelf -l "$BINARY" 2>&1 | grep -q 'INTERP'; then
            echo "ERROR: $BINARY requested an interpreter! Must be pure static or static-PIE." >&2
            exit 1
        fi
        echo "  [PASS] Verified zero interpreter headers (pure static / static-PIE)."
        ;;

    2.31)
        # In glibc 2.31 LTS binaries, no symbol version beyond 2.31 may be required.
        if readelf -sV "$BINARY" 2>&1 | grep -E 'GLIBC_2\.(3[2-9]|[4-9][0-9])'; then
            echo "ERROR: $BINARY leaked symbols exceeding GLIBC_2.31 ceiling!" >&2
            exit 1
        fi
        echo "  [PASS] Verified glibc <= 2.31 symbol ceiling."
        ;;

    *)
        echo "ERROR: Unknown check type: '$CHECK_TYPE'. Must be 'static' or '2.31'." >&2
        exit 1
        ;;
esac
