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
| `ck-mc` | 0.49.10 (upstream tag `v0.49.10`, `a3e7ac74`; Arcus `ck-mc` seq 11, published 2026-10-18) | Upstream publishes no current binary (its release is a stale alpha). Built unmodified from the tag. The Arcus envelope carries four targets (darwin-arm64, darwin-x64 built and signed here; linux-x64, linux-arm64 built by Cloudhome); the darwin-arm64 archive's binary is byte-identical to the one placed on this Mac. **`arcus install ck-mc` FAILS on every platform as of 2026-10-18** (the signed envelope's archive URLs are the GitHub release base plus a gateway path; reported to Cloudhome 8293c528, needs a re-sign at a new sequence), so do not use the Arcus catalog entry yet. Placed 2026-10-18 with `place-ck-mc.sh --require-epochs-unchanged` (stores context 95/95, store 63/63, epochs unchanged); rollback binaries in `~/.local/share/cortexkit/bin/staging/`. | Arcus package `ck-mc` (Cloudhome builds Linux, we sign Darwin) |
| `ck-entorhinal` | 0.1.20 | Upstream HEAD, unmodified. Nothing is published. | us |
| `aft` | 0.58.2 (fork) | UltraCreative fork: SubC federation, LSP and GDScript changes that upstream 0.60.0 lacks | the `aft` project (Arcus) |
| `ck-uc-discussions` | 0.1.1 (Arcus package `uc-discussions` seq 3, published 2026-10-18; `arcus install` verified 2026-10-18; binary byte-identical to the installed one) | `modules/uc-discussions/` | us |
| `lore-thalamus` | n/a | Lore, **deprecated**; left running, never updated (session identity for non-OpenCode harnesses; no published equivalent) | n/a |
| `lore-broca`, `lore-wake`, `lore-mcp-gateway` | n/a | **Retired 2026-10-17** (operator-approved): broca unused with `transform_mode: ts`, wake replaced by `project_room` + `ck-uc-discussions`, mcp-gateway had no clients. Binaries archived in `~/.local/state/lore-retired-20261017-073526/` | n/a |

Daemon configuration that replaces our retired patches:

- `XDG_RUNTIME_DIR=$HOME/.local/share/cortexkit/run` in `~/Library/LaunchAgents/cortexkit.subc.plist`, so the stock
  daemon writes its connection file there and not to the macOS temp directory.
- `route_bind_relay_timeout_ms: 30000` for `aft` in `~/.config/cortexkit/subc.jsonc`.
