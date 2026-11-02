#!/usr/bin/env bash
set -euo pipefail

# fleet/upgrade-ck.sh — Safe wrapper around `ck upgrade` preserving custom LaunchAgent plist
#
# CortexKit's `ck upgrade` downloads and verifies fresh binaries into ~/.local/share/cortexkit/bin.
# When attempting `service-manager-restart`, it detects that ~/Library/LaunchAgents/cortexkit.subc.plist
# has user-edited contents (specifically XDG_RUNTIME_DIR to protect connection files from /var/folders/ cleanup)
# and intentionally refuses to overwrite the plist.
#
# This script executes `ck upgrade`, mirrors upgraded binaries to ~/.local/bin, and safely kickstarts launchd.

echo "==> Step 1: Running upstream ck upgrade..."
# Allow exit code from refusal during service restart
ck upgrade "$@" || true

echo "==> Step 2: Syncing upgraded binaries to ~/.local/bin/..."
if [ -f "$HOME/.local/share/cortexkit/bin/ck-subc" ]; then
    cp "$HOME/.local/share/cortexkit/bin/ck-subc" "$HOME/.local/bin/ck-subc"
    echo "  Synced ck-subc -> ~/.local/bin/ck-subc ($("$HOME/.local/bin/ck-subc" --version 2>/dev/null || echo 'unknown'))"
fi

if [ -f "$HOME/.local/share/cortexkit/bin/ck" ]; then
    cp "$HOME/.local/share/cortexkit/bin/ck" "$HOME/.local/bin/ck"
    echo "  Synced ck -> ~/.local/bin/ck ($("$HOME/.local/bin/ck" --version 2>/dev/null || echo 'unknown'))"
fi

echo "==> Step 3: Safely restarting ck-subc daemon via launchd (preserving custom plist)..."
launchctl kickstart -k gui/$(id -u)/cortexkit.subc

echo "==> Step 4: Waiting for daemon listener and verifying health..."
sleep 2
ck health

echo "==> Upgrade workflow complete!"
