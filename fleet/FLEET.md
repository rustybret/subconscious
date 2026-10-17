# Fleet module sources

What runs under the local `ck-subc`, where each binary comes from, and who builds it. Update this
file in the same commit that changes a module's source. Verified 2026-10-17.

| Module | Version | Source | Built by |
| --- | --- | --- | --- |
| `ck-subc`, `ck` | 0.20.83 | CortexKit official release (`subc-core-v0.20.83`), signed | CortexKit |
| `ck-subc-mcp` | 0.20.83 | CortexKit official release | CortexKit |
| `ck-insula` | 0.1.1 | CortexKit official release | CortexKit |
| `ck-synapse` | 0.1.0-alpha.2 | CortexKit official release | CortexKit |
| `ck-claustrum` | 0.1.3 | Source build of the `claustrum` checkout. The published 0.1.3 is rejected by the daemon (no launch-nonce handshake). Asked `claustrum` for a fixed release. | us, until fixed |
| `ck-mc` | upstream tag `v0.49.1` | Upstream publishes no current binary (its release is a stale alpha). Built unmodified from the upstream tag. | us today; Cloudhome target |
| `ck-entorhinal` | 0.1.20 | Upstream HEAD, unmodified. Nothing is published. | us |
| `aft` | 0.58.2 (fork) | UltraCreative fork: SubC federation, LSP and GDScript changes that upstream 0.60.0 lacks | the `aft` project (Arcus) |
| `ck-uc-discussions` | 0.1.0 | `modules/uc-discussions/` | us |
| `lore-thalamus`, `lore-broca`, `lore-wake`, `lore-mcp-gateway` | n/a | Lore, **deprecated**; left running, not updated | n/a |

Daemon configuration that replaces our retired patches:

- `XDG_RUNTIME_DIR=$HOME/.local/share/cortexkit/run` in `~/Library/LaunchAgents/cortexkit.subc.plist`, so the stock
  daemon writes its connection file there and not to the macOS temp directory.
- `route_bind_relay_timeout_ms: 30000` for `aft` in `~/.config/cortexkit/subc.jsonc`.
