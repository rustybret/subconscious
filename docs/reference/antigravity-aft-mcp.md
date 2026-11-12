# Antigravity (agy) and AFT over MCP: operator reference

Status: **gateway live, tools discovered by agy, execution blocked by provider preset requirement.**
This document records verified CLI and config contracts, empirical test proofs, and the exact
wire-level contract blocker identified during live execution.
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
- The gateway that the shim connects to is the supervised module `ck-subc-mcp`. It is configured
  in `subc.jsonc` and running healthy under daemon supervision (`ck module status ck-subc-mcp`).
- `agy` is authenticated and operational (`agy models` lists Gemini, Claude, and GPT models).
- `agy` registers the MCP server via `agy mcp add aft -- /Users/brethoffman/.local/bin/ck-subc-mcp shim --harness agy`.
- Project-local provider filtering (`.cortexkit/mcp.jsonc`) is active and restricts exposed tools
  exclusively to `aft` (24 tools), disabling `entorhinal`, `fusiform`, and `uc-discussions`.
- Live headless discovery succeeds: `agy -p "What tools from the aft server are available to you?"`
  spawns the shim, connects to `ck-subc-mcp`, and enumerates all 24 `aft_*` tools.
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

## Identified execution blocker

1. **Tool call preset requirement**: During live invocation of any `aft_*` tool, AFT 0.64.0 rejects
   the call with:
   `subc route tool call failed: invalid_request: a tool call on a route bound under a scope must name its catalog preset ("head", "worker" or "reader")`.
2. `ck-subc-mcp` 0.21.3 explicitly sets `preset: None` on `ToolCallRequest` (`crates/subc-mcp/src/main.rs:5768`)
   because generic MCP callers do not supply a session preset.
3. Resolution requires either AFT defaulting absent presets on scoped MCP routes to a safe role
   (e.g. `worker` or `reader`), or `ck-subc-mcp` providing a configuration mechanism to declare a preset.
## Proof checklist

Mark an item only after running the command and reading its output.

- [x] `.agents/mcp_config.json` written; `jq` validation true; 24 `aft_` names in `enabledTools`.
- [x] `agy --version` reports 1.4.9.
- [x] `agy --help` lists `--output-format` with `text`, `json`, and `stream-json`.
- [x] agy loads the MCP server and discovers all 24 `aft_*` tools in headless mode.
- [x] The gateway supervision entry for `ck-subc-mcp` is configured and running (`pid 32184`).
- [x] `ck-subc-mcp shim --harness agy` starts, performs handshake, and lists tools over stdio.
- [ ] Tool execution succeeds (currently blocked by AFT preset requirement on scoped route).
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
