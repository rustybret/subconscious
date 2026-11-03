#!/usr/bin/env bash
set -euo pipefail

# fleet/upgrade-ck.sh — Safe wrapper around `ck upgrade` preserving LaunchAgent plist
#
# DANGER NOTE:
# The ck-subc daemon supervises all fleet modules (aft, magic-context, synapse, etc.).
# Terminating or restarting the daemon abruptly severs open socket connections to
# active OpenCode servers and plugins, causing running sessions and agents to crash/abort.
#
# This script:
# 1. Runs `ck upgrade` to update binaries in ~/.local/share/cortexkit/bin/.
# 2. Syncs updated binaries to ~/.local/bin/.
# 3. Checks for running OpenCode instances before touching the daemon.
# 4. Refuses to restart the daemon if active OpenCode instances are detected,
#    unless the operator explicitly passes --force.

FORCE_RESTART=false
for arg in "$@"; do
    if [ "$arg" = "--force" ] || [ "$arg" = "-f" ]; then
        FORCE_RESTART=true
    fi
done

echo "==> Step 1: Running upstream ck upgrade..."
# Allow exit code from refusal during service restart (plist user-edit protection)
ck upgrade "$@" 2>&1 || true

echo "==> Step 2: Syncing upgraded binaries to ~/.local/bin/..."
if [ -f "$HOME/.local/share/cortexkit/bin/ck-subc" ]; then
    cp "$HOME/.local/share/cortexkit/bin/ck-subc" "$HOME/.local/bin/ck-subc"
    echo "  Synced ck-subc -> ~/.local/bin/ck-subc ($("$HOME/.local/bin/ck-subc" --version 2>/dev/null || echo 'unknown'))"
fi

if [ -f "$HOME/.local/share/cortexkit/bin/ck" ]; then
    cp "$HOME/.local/share/cortexkit/bin/ck" "$HOME/.local/bin/ck"
    echo "  Synced ck -> ~/.local/bin/ck ($("$HOME/.local/bin/ck" --version 2>/dev/null || echo 'unknown'))"
fi

echo "==> Step 3: Checking for active OpenCode instances..."
ACTIVE_OPENCODE=$(pgrep -lf "opencode" | grep -v "upgrade-ck" || true)

if [ -n "$ACTIVE_OPENCODE" ]; then
    echo "⚠️  WARNING: Active OpenCode processes detected:"
    echo "$ACTIVE_OPENCODE" | sed 's/^/    /'
    echo ""
    if [ "$FORCE_RESTART" = false ]; then
        echo "⛔ Daemon restart ABORTED to protect active sessions and running agents."
        echo "   Restarting ck-subc will sever sockets to aft/magic-context and crash running sessions."
        echo "   To apply the new binary when sessions are idle or completed, run:"
        echo "       launchctl kickstart -k gui/\$(id -u)/cortexkit.subc"
        echo "   Or re-run this script with --force to restart immediately."
        exit 0
    else
        echo "⚠️  --force specified. Proceeding with daemon restart..."
    fi
fi

echo "==> Step 4: Safely restarting ck-subc daemon via launchd..."
launchctl kickstart -k gui/$(id -u)/cortexkit.subc

echo "==> Step 5: Waiting for daemon listener and verifying health..."
sleep 2
ck health

echo "==> Upgrade workflow complete!"
