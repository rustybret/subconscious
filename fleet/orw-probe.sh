#!/usr/bin/env bash
# orw-probe.sh — installed-vs-latest drift tripwire for magic-context (Tier 1 plugin, Tier 2 ck-mc daemon)
#
# Exit codes: 0 up to date, 1 drift in any checked tier, 2 refused (npm registry unreachable).
# Tier 2 compares the installed ck-mc build SHA with the commit of the upstream release tag v<npm latest>.
# It is skipped (reported, not failed) when the binary has no build SHA or the upstream tag cannot be resolved.
# No local checkout is consulted: ck-mc is built from the unmodified upstream tag.
#
# Env overrides:
#   OPENCODE_CONFIG_FILE  opencode.json holding the plugin pin
#   CK_MC_BIN             installed ck-mc binary (default ~/.local/share/cortexkit/bin/ck-mc)
#   MC_UPSTREAM_URL       git remote holding the release tags (default https://github.com/cortexkit/magic-context.git)
set -euo pipefail

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

# 2. Tier 2: installed ck-mc build SHA vs the commit of the upstream release tag v<npm latest>
CK_MC_BIN="${CK_MC_BIN:-$HOME/.local/share/cortexkit/bin/ck-mc}"
MC_UPSTREAM_URL="${MC_UPSTREAM_URL:-https://github.com/cortexkit/magic-context.git}"

if [ ! -x "$CK_MC_BIN" ]; then
    echo "ck-mc: SKIPPED (binary not found: $CK_MC_BIN)"
else
    INSTALLED_SHA=$("$CK_MC_BIN" --version 2>/dev/null | grep -oE '[0-9a-f]{40}' | head -n1 || true)
    TAG_REFS=$(git ls-remote "$MC_UPSTREAM_URL" "refs/tags/v$NPM_LATEST" "refs/tags/v$NPM_LATEST^{}" 2>/dev/null || true)
    # An annotated tag lists the tag object and then the peeled commit (^{}); the peeled line wins.
    TAG_SHA=$(printf '%s\n' "$TAG_REFS" | awk '/\^\{\}$/ {print $1; found=1} END {if (!found) exit 1}' 2>/dev/null || true)
    [ -n "$TAG_SHA" ] || TAG_SHA=$(printf '%s\n' "$TAG_REFS" | awk 'NF {print $1; exit}')
    if [ -z "$INSTALLED_SHA" ]; then
        echo "ck-mc: SKIPPED (no build SHA in --version output)"
    elif [ -z "$TAG_SHA" ]; then
        echo "ck-mc: SKIPPED (upstream tag v$NPM_LATEST not resolvable at $MC_UPSTREAM_URL)"
    elif [ "$INSTALLED_SHA" != "$TAG_SHA" ]; then
        echo "ck-mc: DRIFT_DETECTED (installed build: ${INSTALLED_SHA:0:9}, upstream v$NPM_LATEST: ${TAG_SHA:0:9})"
        DRIFT=1
    else
        echo "ck-mc: UP_TO_DATE (${TAG_SHA:0:9}, upstream v$NPM_LATEST)"
    fi
fi

exit "$DRIFT"
