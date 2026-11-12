# Antigravity (agy) and AFT over MCP: operator reference

Status: **config prepared, not discovered, not working.** This document records verified CLI and config
contracts and the blockers that remain. It does not claim that agy loads the project config or can call
live AFT MCP tools. No live AFT MCP success has been demonstrated.

## Verified facts

- Installed agy is 1.4.9.
- `agy` supports `--output-format` with `text`, `json`, and `stream-json`.
- `--print-timeout` defaults to `0s`, which means no timeout. An older note said plain text only with a
  5 minute default. That note is wrong for this install.
- Project MCP configuration file: `.agents/mcp_config.json`. It supports an `enabledTools` allowlist.
  Source: https://antigravity.google/docs/mcp/
- The stdio entry uses the absolute executable returned by `command -v ck-subc-mcp`,
  with `args: [shim, --harness, agy]` and `cwd` set to the workspace. The file lists 24
  `enabledTools` names, all `aft_` tools from the `ck --json`
  catalog. `jq` validation of the file returns true.
- The gateway that the shim connects to is the supervised module `ck-subc-mcp`. It is absent on this
  machine. Its supervision entry needs operator configuration. Do not launch it ad hoc as a standalone
  process.
- No agy user login is present. `agy models` shows no authenticated account.
- `agy mcp list` still reports "No MCP servers configured" after the local config was created. This may
  mean the list shows only global servers. Project loading at runtime is unverified. Do not conclude from
  this that workspace config is unsupported.
- `aft_inspect` on the JSON file reports an unknown producer, not clean diagnostics. That is no
  diagnostics coverage, not a pass.

## Configuration contracts

- **AFT user tier:** the OpenCode TypeScript plugin needs an explicit absolute path to the current daemon
  connection file. An empty value means standalone mode, which is not the integration path.
- **agy shim:** the shim does not read an `aft.jsonc` transport selector. It connects to the gateway. Do
  not configure AFT transport for the shim.
- **Legacy path must stay absent on this fleet:** with the stock macOS launchd definition,
  a stale `~/.local/share/cortexkit/run/subc-connection.json` overrides discovery and breaks clients.
  The current stock runtime writes `$TMPDIR/subc-<uid>.connection.json`.
- **Protected config floor:** applies independently of OpenCode's `external_directory` setting. Never
  bypass it through a shell command.
- **`enabledTools` is not a security boundary:** it hides unrelated tool schemas from the agent.
  Provider admission and AFT's permissions remain separate from schema visibility; a filtered
  catalog alone does not prove authorization.
- **Caller trust and grants:** an unknown caller's trust and grants need live proof. Do not claim that a
  bash call succeeds through AFT until that proof exists.
- **Accounts:** one account per project is an operator preference. It is not verified native account
  isolation. Do not redirect `HOME` and do not copy tokens between accounts.

## Blockers

1. The `ck-subc-mcp` gateway supervision entry is not configured. The shim has nothing to connect to.
2. agy has no user login.
3. Runtime loading of `.agents/mcp_config.json` is unverified. `agy mcp list` still shows no servers.
4. The 24 AFT tools have not been called through agy, so no tool call has run.
5. Caller trust and grants for agy have not been checked live.

## Proof checklist

Mark an item only after running the command and reading its output.

- [x] `.agents/mcp_config.json` written; `jq` validation true; 24 `aft_` names in `enabledTools`.
- [x] `agy --version` reports 1.4.9.
- [x] `agy --help` lists `--output-format` with `text`, `json`, and `stream-json`.
- [ ] agy loads the project config at runtime (not shown by `agy mcp list` yet).
- [ ] The gateway supervision entry for `ck-subc-mcp` is configured by the operator and running.
- [ ] `ck-subc-mcp shim --harness agy` starts and lists tools over stdio.
- [ ] One allowlisted AFT tool call succeeds from agy, with its output captured.
- [ ] `~/.local/share/cortexkit/run/subc-connection.json` is absent.
- [x] The current connection file exists at `$TMPDIR/subc-<uid>.connection.json`.

## Sources

- https://antigravity.google/docs/mcp/ (project MCP config and `enabledTools`)
- https://antigravity.google/docs/cli/headless/ (headless output and approval behavior)
- https://github.com/cortexkit/subconscious/blob/subc-core-v0.21.3/crates/subc-mcp/src/main.rs
  (gateway modes, provider filtering, and exported tool namespaces)

The workspace config contains local executable and working-directory paths and remains gitignored
under `.agents/`. Recreate it per workstation rather than publishing personal machine paths. No
gateway, model session, background process, or login was started during this preparation.
