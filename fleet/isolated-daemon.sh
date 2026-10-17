#!/usr/bin/env bash
# Run the STOCK published ck-subc in a throwaway sandbox with one or more modules, to test a
# module build against the daemon the fleet actually runs, without touching live state.
#
# Isolation: temp HOME/XDG dirs, SUBC_PORT=0 (random loopback port), its own connection file and
# SQLite store, env wiped (env -i). The live daemon, its connection file and ~/.config/cortexkit
# are never read or written. Everything is removed on exit unless --keep is given.
#
#   fleet/isolated-daemon.sh --module aft=/path/to/aft [--module name=/path ...]
#       [--reserved aft] [--bind-timeout-ms 30000] [--daemon /path/to/ck-subc]
#       [--settle 25] [--hold 0] [--keep]
#
# --module NAME=PATH   module to supervise (repeatable). NAME is its subc module_id.
# --reserved NAME      mark NAME reserved (launch-nonce auth); repeat per module. aft, magic-context,
#                      uc-discussions, entorhinal, synapse, claustrum are reserved on the live fleet.
# --bind-timeout-ms N  route_bind_relay_timeout_ms for every module (default 30000, what the fleet ships).
# --daemon PATH        ck-subc to test (default ~/.local/share/cortexkit/bin/ck-subc; its --version is printed).
# --settle SECS        wait for modules to register before the first report (default 25).
# --hold SECS          keep the sandbox up this long AFTER the report so you can drive a client at it
#                      (the connection file path is printed); default 0.
# Exit: 0 all modules registered with protocol v2 and zero bind timeouts/breaker trips; 1 otherwise.
set -u
DAEMON="$HOME/.local/share/cortexkit/bin/ck-subc"; CK="$HOME/.local/share/cortexkit/bin/ck"
TIMEOUT=30000; SETTLE=25; HOLD=0; KEEP=0; MODS=(); RESERVED=()
while [ $# -gt 0 ]; do case "$1" in
  --module) MODS+=("$2"); shift 2;; --reserved) RESERVED+=("$2"); shift 2;;
  --bind-timeout-ms) TIMEOUT="$2"; shift 2;; --daemon) DAEMON="$2"; shift 2;;
  --settle) SETTLE="$2"; shift 2;; --hold) HOLD="$2"; shift 2;; --keep) KEEP=1; shift;;
  -h|--help) sed -n 2,22p "$0"; exit 0;; *) echo "unknown option: $1" >&2; exit 2;; esac; done
[ ${#MODS[@]} -gt 0 ] || { echo "need at least one --module NAME=PATH" >&2; exit 2; }
[ -x "$DAEMON" ] || { echo "daemon not executable: $DAEMON" >&2; exit 2; }
[ -x "$CK" ] || CK="$(command -v ck || true)"

T="$(mktemp -d /tmp/subc-iso.XXXXXX)"; mkdir -p "$T"/{run,data,config/cortexkit,home}
DPID=""
cleanup() {
  # The daemon drains and RESPAWNS its modules after SIGTERM, so killing modules first leaves a fresh orphan
  # behind once the daemon exits. Stop the daemon, wait for it, then sweep anything still holding the sandbox.
  if [ -n "$DPID" ]; then kill "$DPID" 2>/dev/null
    for _ in $(seq 1 45); do kill -0 "$DPID" 2>/dev/null || break; sleep 1; done
    kill -9 "$DPID" 2>/dev/null; fi
  pkill -f "$T/" 2>/dev/null; sleep 1; pkill -9 -f "$T/" 2>/dev/null
  [ "$KEEP" = 1 ] && echo "kept: $T" || rm -rf "$T"; }
trap cleanup EXIT

is_reserved() { for r in "${RESERVED[@]:-}"; do [ "$r" = "$1" ] && return 0; done; return 1; }
{ echo '{ "version": 1, "storage": { "backend": "sqlite" }, "modules": {'
  n=0; for m in "${MODS[@]}"; do name="${m%%=*}"; path="${m#*=}"
    [ -x "$path" ] || { echo "module binary not executable: $path" >&2; exit 2; }
    [ $n -gt 0 ] && echo ","; n=$((n+1))
    printf '  "%s": { "program": "%s", "route_bind_relay_timeout_ms": %s' "$name" "$path" "$TIMEOUT"
    is_reserved "$name" && printf ', "reserved": true'; printf ' }'; done
  echo; echo '} }'; } > "$T/config/cortexkit/subc.jsonc"

echo "daemon : $("$DAEMON" --version 2>&1 | head -1)  ($DAEMON)"
echo "sandbox: $T   bind timeout: ${TIMEOUT}ms"
( cd "$T" && env -i PATH="$PATH" HOME="$T/home" XDG_RUNTIME_DIR="$T/run" XDG_DATA_HOME="$T/data" \
    XDG_CONFIG_HOME="$T/config" SUBC_PORT=0 nohup "$DAEMON" > "$T/daemon.out" 2>&1 & echo $! > "$T/pid" )
DPID="$(cat "$T/pid")"; sleep "$SETTLE"
CONN="$T/run/subc-connection.json"; LOG="$(ls "$T"/data/cortexkit/run/logs/subc.*.log 2>/dev/null | head -1)"
[ -f "$CONN" ] || { echo "FAIL: daemon wrote no connection file"; tail -5 "$T/daemon.out"; exit 1; }
echo "connection file: $CONN   (point a client at this, not at ~/.local/share/cortexkit/run)"
report() {
  echo "--- registration"; grep -h "module registered" "$LOG" 2>/dev/null | sed -E 's/^[^ ]+ +//' | cut -c1-140
  echo "--- health"; "$CK" --subc "$CONN" health 2>&1 | cut -c1-110
  TO=$(grep -c -E "route.bind relay timed out|breaker|target_binds_full|module_timeout" "$LOG" 2>/dev/null)
  RO=$(grep -c "route.open accepted" "$LOG" 2>/dev/null)
  echo "--- route.open accepted=$RO  bind timeouts/breaker/binds_full/module_timeout=$TO"; }
report
if [ "$HOLD" -gt 0 ]; then echo "holding ${HOLD}s for a client run against $CONN ..."; sleep "$HOLD"; report; fi
BAD=0
for m in "${MODS[@]}"; do name="${m%%=*}"
  grep -q "module registered module_id=$name .*negotiated_ver=2" "$LOG" || { echo "FAIL: $name did not register at protocol v2"; BAD=1; }
  # Registering is not enough: a module that registers and then exits is a crash loop, not a pass.
  CR=$(grep -c "supervised module exited abnormally.*module_id=$name" "$LOG" 2>/dev/null)
  [ "${CR:-0}" -eq 0 ] || { echo "FAIL: $name exited abnormally ${CR}x (crash loop); see $T/data/cortexkit/run/logs/$name.stderr.log (rerun with --keep)"
    tail -2 "$T/data/cortexkit/run/logs/$name.stderr.log" 2>/dev/null | cut -c1-220; BAD=1; }
  kill -0 "$(pgrep -f "^$(echo "${m#*=}" | sed 's/[.[\*^$]/\\&/g')" | head -1)" 2>/dev/null || { echo "FAIL: $name process is not running at the end"; BAD=1; }
done
[ "${TO:-1}" -eq 0 ] || { echo "FAIL: bind timeouts/breaker trips seen"; BAD=1; }
[ $BAD -eq 0 ] && echo "RESULT: PASS" || echo "RESULT: FAIL"; exit $BAD
