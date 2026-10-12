#!/usr/bin/env bash
# orw-probe.sh — installed-vs-latest drift tripwire for magic-context (Tier 1 plugin, Tier 2 ck-mc daemon)
#
# Exit codes: 0 up to date, 1 drift in any checked tier, 2 refused (npm registry unreachable).
# Tier 2 is skipped (reported, not failed) when the ck-mc binary or the magic-context checkout is absent.
#
# Env overrides:
#   OPENCODE_CONFIG_FILE  opencode.json holding the plugin pin
#   CK_MC_BIN             installed ck-mc binary (default ~/.local/share/cortexkit/bin/ck-mc)
#   MC_REPO               magic-context checkout whose HEAD is the build target
#                         (default: sibling ../magic-context of this repo)
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
DRIFT=0

# 1. Tier 1: probe npm registry for latest @cortexkit/opencode-magic-context
NPM_LATEST=$(curl -fsSL --connect-timeout 5 --max-time 10 https://registry.npmjs.org/@cortexkit/opencode-magic-context 2>/dev/null | jq -r '."dist-tags".latest // empty' || true)

if [ -z "$NPM_LATEST" ]; then
    echo "orw-probe: REFUSED (npm registry unreachable)" >&2
    exit 2
fi

CONFIG_DIR="${XDG_CONFIG_HOME:-$HOME/.config}/opencode"
PKG_JSON="${OPENCODE_CONFIG_FILE:-$CONFIG_DIR/opencode.json}"
PINNED_VER=$(grep -oE "@cortexkit/opencode-magic-context@[0-9]+\.[0-9]+\.[0-9]+" "$PKG_JSON" 2>/dev/null | cut -d'@' -f3 || true)

echo "magic-context npm latest: $NPM_LATEST"
echo "magic-context local pin:  ${PINNED_VER:-latest}"

if [ -n "$PINNED_VER" ] && [ "$PINNED_VER" != "$NPM_LATEST" ]; then
    echo "STATUS: DRIFT_DETECTED (installed: $PINNED_VER, latest: $NPM_LATEST)"
    DRIFT=1
else
    echo "STATUS: UP_TO_DATE ($NPM_LATEST)"
fi

# 2. Tier 2: installed ck-mc build SHA vs magic-context checkout HEAD
CK_MC_BIN="${CK_MC_BIN:-$HOME/.local/share/cortexkit/bin/ck-mc}"
MC_REPO="${MC_REPO:-$SCRIPT_DIR/../../../magic-context}"

if [ ! -x "$CK_MC_BIN" ]; then
    echo "ck-mc: SKIPPED (binary not found: $CK_MC_BIN)"
elif ! git -C "$MC_REPO" rev-parse --verify HEAD >/dev/null 2>&1; then
    echo "ck-mc: SKIPPED (magic-context checkout not found: $MC_REPO)"
else
    INSTALLED_SHA=$("$CK_MC_BIN" --version 2>/dev/null | grep -oE '[0-9a-f]{40}' | head -n1 || true)
    HEAD_SHA=$(git -C "$MC_REPO" rev-parse HEAD)
    if [ -z "$INSTALLED_SHA" ]; then
        echo "ck-mc: SKIPPED (no build SHA in --version output)"
    elif [ "$INSTALLED_SHA" != "$HEAD_SHA" ]; then
        echo "ck-mc: DRIFT_DETECTED (installed build: ${INSTALLED_SHA:0:9}, checkout HEAD: ${HEAD_SHA:0:9})"
        DRIFT=1
    else
        echo "ck-mc: UP_TO_DATE (${HEAD_SHA:0:9})"
    fi
fi

exit "$DRIFT"
