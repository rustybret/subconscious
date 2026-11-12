#!/usr/bin/env bash
# Run the STOCK published ck-subc in a throwaway sandbox with one or more modules, to test a
# module build against the daemon the fleet actually runs, without touching live state.
#
# Isolation: temp HOME/XDG dirs, SUBC_PORT=0 (random loopback port), its own connection file and
# SQLite store, env wiped (env -i). The live daemon, its connection file and ~/.config/cortexkit
# are never read or written. Everything is removed on exit (also on Ctrl-C, TERM, HUP) unless --keep.
#
#   fleet/isolated-daemon.sh --module aft=/path/to/aft [--module name=/path ...]
#       [--reserved aft] [--bind-timeout-ms 30000] [--daemon /path/to/ck-subc]
#       [--allow-unhealthy NAME] [--settle 25] [--hold 0] [--keep]
#
# --module NAME=PATH   module to supervise (repeatable). NAME is its subc module_id.
# --reserved NAME      mark NAME reserved (launch-nonce auth); repeat per module. aft, magic-context,
#                      uc-discussions, entorhinal, synapse, claustrum are reserved on the live fleet.
# --bind-timeout-ms N  route_bind_relay_timeout_ms for every module (default 30000, what the fleet ships).
# --daemon PATH        ck-subc to test (default ~/.local/share/cortexkit/bin/ck-subc; its --version is printed).
# --allow-unhealthy N  accept a non-ok `ck health` line for module N (repeatable), e.g. magic-context and
#                      uc-discussions, whose real stores the sandbox cannot open. Reported as UNTESTED.
# --settle SECS        wait for modules to register before the first report (default 25).
# --hold SECS          keep the sandbox up this long AFTER the report so you can drive a client at it
#                      (the connection file path is printed); default 0.
# Exit: 0 all modules registered with protocol v2, are still running, report healthy (or are allowed
# not to), and zero bind timeouts/breaker trips; 1 otherwise.
set -u
DAEMON="$HOME/.local/share/cortexkit/bin/ck-subc"; CK="$HOME/.local/share/cortexkit/bin/ck"
TIMEOUT=30000; SETTLE=25; HOLD=0; KEEP=0; MODS=(); RESERVED=(); ALLOW=()
while [ $# -gt 0 ]; do case "$1" in
  --module) MODS+=("$2"); shift 2;; --reserved) RESERVED+=("$2"); shift 2;;
  --bind-timeout-ms) TIMEOUT="$2"; shift 2;; --daemon) DAEMON="$2"; shift 2;;
  --allow-unhealthy) ALLOW+=("$2"); shift 2;;
  --settle) SETTLE="$2"; shift 2;; --hold) HOLD="$2"; shift 2;; --keep) KEEP=1; shift;;
  -h|--help) sed -n 2,25p "$0"; exit 0;; *) echo "unknown option: $1" >&2; exit 2;; esac; done
[ ${#MODS[@]} -gt 0 ] || { echo "need at least one --module NAME=PATH" >&2; exit 2; }
[ -x "$DAEMON" ] || { echo "daemon not executable: $DAEMON" >&2; exit 2; }
abs() { case "$1" in /*) echo "$1";; *) echo "$PWD/$1";; esac; }   # the daemon starts inside $T
DAEMON="$(abs "$DAEMON")"
[ -x "$CK" ] || CK="$(command -v ck || true)"

T="$(mktemp -d /tmp/subc-iso.XXXXXX)"; mkdir -p "$T"/{run,data,config/cortexkit,home}
DPID=""
# Every sandbox process carries XDG_RUNTIME_DIR=$T/run in its environment (the daemon gets it from env -i
# and passes it to its modules); nothing else does, so `ps -E` finds them even after they are reparented.
# The marker is assembled inside awk, so no argv or environment (awk's included) holds it verbatim.
sandbox_pids() { ps -axeww -o pid=,command= | awk -v d="$T" 'index($0 " ", " XDG_RUNTIME_DIR=" d "/run ") { print $1 }'; }
cleanup() {
  # The daemon drains and RESPAWNS its modules after SIGTERM, so killing modules first leaves a fresh orphan
  # behind once the daemon exits. Stop the daemon, wait for it, then sweep anything still holding the sandbox.
  if [ -n "$DPID" ]; then kill "$DPID" 2>/dev/null
    for _ in $(seq 1 45); do kill -0 "$DPID" 2>/dev/null || break; sleep 1; done
    kill -0 "$DPID" 2>/dev/null && kill -9 "$DPID" 2>/dev/null; fi
  LEFT="$(sandbox_pids)"
  if [ -n "$LEFT" ]; then kill $LEFT 2>/dev/null; sleep 2; LEFT="$(sandbox_pids)"; [ -z "$LEFT" ] || kill -9 $LEFT 2>/dev/null; sleep 1; fi
  LEFT="$(sandbox_pids)"; [ -z "$LEFT" ] || echo "WARNING: sandbox processes still running: $LEFT" >&2
  [ "$KEEP" = 1 ] && echo "kept: $T" || rm -rf "$T"; }
trap cleanup EXIT
trap 'exit 130' INT; trap 'exit 143' TERM; trap 'exit 129' HUP

in_list() { local x="$1"; shift; for r in "$@"; do [ "$r" = "$x" ] && return 0; done; return 1; }
{ echo '{ "version": 1, "storage": { "backend": "sqlite" }, "modules": {'
  n=0; for m in "${MODS[@]}"; do name="${m%%=*}"; path="$(abs "${m#*=}")"
    [ -x "$path" ] || { echo "module binary not executable: $path" >&2; exit 2; }
    [ $n -gt 0 ] && echo ","; n=$((n+1))
    printf '  "%s": { "program": "%s", "route_bind_relay_timeout_ms": %s' "$name" "$path" "$TIMEOUT"
    in_list "$name" "${RESERVED[@]:-}" && printf ', "reserved": true'; printf ' }'; done
  echo; echo '} }'; } > "$T/config/cortexkit/subc.jsonc"

echo "daemon : $("$DAEMON" --version 2>&1 | head -1)  ($DAEMON)"
echo "sandbox: $T   bind timeout: ${TIMEOUT}ms"
# exec, so $! is the daemon itself and not a forking subshell (bash 3.2 does not optimise the fork away).
( cd "$T" && exec env -i PATH="$PATH" HOME="$T/home" XDG_RUNTIME_DIR="$T/run" XDG_DATA_HOME="$T/data" \
    XDG_CONFIG_HOME="$T/config" SUBC_PORT=0 nohup "$DAEMON" > "$T/daemon.out" 2>&1 ) &
DPID=$!
sleep "$SETTLE" & wait $!
CONN="$T/run/subc-connection.json"; LOG="$(ls "$T"/data/cortexkit/run/logs/subc.*.log 2>/dev/null | head -1)"
[ -f "$CONN" ] || { echo "FAIL: daemon wrote no connection file"; tail -5 "$T/daemon.out"; exit 1; }
echo "connection file: $CONN   (point a client at this, not at ~/.local/share/cortexkit/run)"
HEALTH=""
report() {
  echo "--- registration"; grep -h "module registered" "$LOG" 2>/dev/null | sed -E 's/^[^ ]+ +//' | cut -c1-140
  echo "--- health"; HEALTH="$("$CK" --subc "$CONN" health 2>&1)"; echo "$HEALTH" | cut -c1-110
  TO=$(grep -c -E "route.bind relay timed out|breaker|target_binds_full|module_timeout" "$LOG" 2>/dev/null)
  RO=$(grep -c "route.open accepted" "$LOG" 2>/dev/null)
  echo "--- route.open accepted=$RO  bind timeouts/breaker/binds_full/module_timeout=$TO"; }
report
if [ "$HOLD" -gt 0 ]; then echo "holding ${HOLD}s for a client run against $CONN ..."; sleep "$HOLD" & wait $!; report; fi
BAD=0
for m in "${MODS[@]}"; do name="${m%%=*}"
  grep -q "module registered module_id=$name .*negotiated_ver=2" "$LOG" || { echo "FAIL: $name did not register at protocol v2"; BAD=1; }
  # Registering is not enough: a module that registers and then exits is a crash loop, not a pass.
  CR=$(grep -c "supervised module exited abnormally.*module_id=$name" "$LOG" 2>/dev/null)
  [ "${CR:-0}" -eq 0 ] || { echo "FAIL: $name exited abnormally ${CR}x (crash loop); see $T/data/cortexkit/run/logs/$name.stderr.log (rerun with --keep)"
    tail -2 "$T/data/cortexkit/run/logs/$name.stderr.log" 2>/dev/null | cut -c1-220; BAD=1; }
  # Only this sandbox daemon's children count; the live fleet may run the same binary path.
  kill -0 "$(pgrep -P "$DPID" -f "^$(abs "${m#*=}" | sed 's/[.[\*^$]/\\&/g')" | head -1)" 2>/dev/null || { echo "FAIL: $name process is not running at the end"; BAD=1; }
  H="$(echo "$HEALTH" | awk -v n="$name" '$1 == n { print $2 }')"
  if [ "$H" != "ok" ]; then
    if in_list "$name" "${ALLOW[@]:-}"; then echo "UNTESTED: $name health is '${H:-missing}' (allowed by --allow-unhealthy)"
    else echo "FAIL: $name health is '${H:-missing}'"; BAD=1; fi
  fi
done
[ "${TO:-1}" -eq 0 ] || { echo "FAIL: bind timeouts/breaker trips seen"; BAD=1; }
[ $BAD -eq 0 ] && echo "RESULT: PASS" || echo "RESULT: FAIL"; exit $BAD
