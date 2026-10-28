# Fleet module sources

What runs under the local `ck-subc`, where each binary comes from, and who builds it. Update this
file in the same commit that changes a module's source. Verified 2026-10-25.

| Module | Version | Source | Built by |
| --- | --- | --- | --- |
| `ck-subc`, `ck` | 0.20.124 | CortexKit official release (`subc-core-v0.20.124`), signed | CortexKit |
| `ck-subc-mcp` | 0.1.42 (`subc-core-v0.20.124`) | CortexKit official release | CortexKit |
| `ck-insula` | 0.1.1 (upstream main `5aa93fae8`, unreleased) | `submodules/insula`, built unmodified. Official v0.1.1 release predates the published-crate migration and crash-looped on stock `ck-subc` 0.20.124 (`invalid envelope header: reserved flag bits set`). Deployed 2026-10-28: 1568 tests pass, isolated sandbox PASS, live healthy (`buildCommit 5aa93fae8083`). Backup: `~/.local/state/insula-backup-20261028-144154`. Return to official release once upstream publishes past v0.1.1. | us |
| `ck-synapse` | 0.1.0-alpha.2 | CortexKit official release | CortexKit |
| `ck-claustrum` | 0.2.0 (official release `v0.2.0`, published 2026-10-22) | CortexKit official release (`v0.2.0`, `ck-claustrum` and `ck-auth`), consumer enrollment, scoped grants, schema 18 forward migration. Deployed 2026-10-25; vault schema 18 active, credentials preserved (backup: `~/.local/state/claustrum-backup-20261025-0.2.0-pre`). Running binary matches official CortexKit release. | CortexKit |
| `ck-mc` | 0.49.20 (upstream tag `v0.49.20`, `3bbf64110`) | Upstream publishes no current binary. Built unmodified from tag on this Mac (MC_BUILD_SHA=3bbf64110e77b9af8148474685972c66bb25a4cc, stripped, adhoc codesign hardened-runtime). Placed 2026-10-25 with `place-ck-mc.sh --require-epochs-unchanged` (stores context 95/95, store 63/63, all four epochs unchanged); verified via `orw-probe.sh` (`UP_TO_DATE`). Arcus Sequence 15 promoted to stable/latest with native Linux receipts (Cloudhome Jenkins #2). Rollback binaries in `~/.local/share/cortexkit/bin/staging/`. | us (Darwin build/placement), Cloudhome (Arcus multi-arch packaging) |
| `ck-entorhinal` | 0.1.46 (submodule commit `84677e1`) | Upstream `origin/main` (`submodules/entorhinal`), unmodified. Built & deployed 2026-10-25; verified `ck-entorhinal 0.1.46` serving `project-identity/v1` and `agent-identity/v1` under `ck-subc` (backup: `~/.local/state/cortexkit-backup-20261025-entorhinal-0.1.20/`). | us |
| `aft` | 0.58.2 (fork) | UltraCreative fork: SubC federation, LSP and GDScript changes that upstream 0.60.0 lacks | the `aft` project (Arcus) |
| `ck-uc-discussions` | 0.1.1+inbox (Arcus package `uc-discussions` seq 3; CLI domain + `agent-inbox/v1`) | `modules/uc-discussions/` (with `~/.local/bin/ck-discussions -> ck-uc-discussions` for `ck discussions`) | us |
| `ck-fusiform`, `ck-models` | 0.1.0 (`fusiform-protocol` 0.30.0 on crates.io; catalog version 1792525097774) | `submodules/fusiform` (`ck models` CLI) | us |
| `ck-astrocyte` | 0.1.0 | `submodules/astrocyte` (AI usage & spend ledger) | us |
| `lore-thalamus` | n/a | Lore, **deprecated**; left running, never updated (session identity for non-OpenCode harnesses; no published equivalent) | n/a |
| `lore-broca`, `lore-wake`, `lore-mcp-gateway` | n/a | **Retired 2026-10-17** (operator-approved): broca unused with `transform_mode: ts`, wake replaced by `project_room` + `ck-uc-discussions`, mcp-gateway had no clients. Binaries archived in `~/.local/state/lore-retired-20261017-073526/` | n/a |

Daemon configuration that replaces our retired patches:

- `XDG_RUNTIME_DIR=$HOME/.local/share/cortexkit/run` in `~/Library/LaunchAgents/cortexkit.subc.plist`, so the stock
  daemon writes its connection file there and not to the macOS temp directory.
- `route_bind_relay_timeout_ms: 30000` for `aft` in `~/.config/cortexkit/subc.jsonc`.
