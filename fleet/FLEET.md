# Fleet module sources

What runs under the local `ck-subc`, where each binary comes from, and who builds it. Update this
file in the same commit that changes a module's source. Verified 2026-10-17.

| Module | Version | Source | Built by |
| --- | --- | --- | --- |
| `ck-subc`, `ck` | 0.20.83 | CortexKit official release (`subc-core-v0.20.83`), signed | CortexKit |
| `ck-subc-mcp` | 0.20.83 | CortexKit official release | CortexKit |
| `ck-insula` | 0.1.1 | CortexKit official release | CortexKit |
| `ck-synapse` | 0.1.0-alpha.2 | CortexKit official release | CortexKit |
| `ck-claustrum` | 0.1.3 (Arcus `ck-claustrum` 0.1.3-ucs.1 seq 1, published 2026-10-17) | Upstream `claustrum` master built unmodified at `76c94df` of the `rustybret/claustrum` fork (branch `sync-upstream-registry` merged to master, registry deps only, tests 1129 passed), darwin-arm64 only, with `ck-auth`. The published CortexKit 0.1.3 predates the launch-nonce fix and is rejected by the daemon. Deployed 2026-10-17 from the earlier `a66ff2c` build; migrated the vault schema 13 to 16 (backup: `~/.local/state/claustrum-backup-20261017-085601`). The running binary is not yet the Arcus-packaged one. | Arcus package `ck-claustrum` (us), until claustrum publishes a fixed release |
| `ck-mc` | upstream tag matching the plugin pin (plugin pinned 0.49.10; installed daemon is still the v0.49.1 build, a7cd854dd, so drift is flagged) | Upstream publishes no current binary (its release is a stale alpha). Built unmodified from the upstream tag. | us today; Cloudhome target |
| `ck-entorhinal` | 0.1.20 | Upstream HEAD, unmodified. Nothing is published. | us |
| `aft` | 0.58.2 (fork) | UltraCreative fork: SubC federation, LSP and GDScript changes that upstream 0.60.0 lacks | the `aft` project (Arcus) |
| `ck-uc-discussions` | 0.1.1 (Arcus package `uc-discussions` seq 3, published 2026-10-18; binary byte-identical to the GitHub release asset and the installed one) | `modules/uc-discussions/` | us |
| `lore-thalamus` | n/a | Lore, **deprecated**; left running, never updated (session identity for non-OpenCode harnesses; no published equivalent) | n/a |
| `lore-broca`, `lore-wake`, `lore-mcp-gateway` | n/a | **Retired 2026-10-17** (operator-approved): broca unused with `transform_mode: ts`, wake replaced by `project_room` + `ck-uc-discussions`, mcp-gateway had no clients. Binaries archived in `~/.local/state/lore-retired-20261017-073526/` | n/a |

Daemon configuration that replaces our retired patches:

- `XDG_RUNTIME_DIR=$HOME/.local/share/cortexkit/run` in `~/Library/LaunchAgents/cortexkit.subc.plist`, so the stock
  daemon writes its connection file there and not to the macOS temp directory.
- `route_bind_relay_timeout_ms: 30000` for `aft` in `~/.config/cortexkit/subc.jsonc`.
