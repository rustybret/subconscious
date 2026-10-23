# Fleet module sources

What runs under the local `ck-subc`, where each binary comes from, and who builds it. Update this
file in the same commit that changes a module's source. Verified 2026-10-17.

| Module | Version | Source | Built by |
| --- | --- | --- | --- |
| `ck-subc`, `ck` | 0.20.83 | CortexKit official release (`subc-core-v0.20.83`), signed | CortexKit |
| `ck-subc-mcp` | 0.20.83 | CortexKit official release | CortexKit |
| `ck-insula` | 0.1.1 | CortexKit official release | CortexKit |
| `ck-synapse` | 0.1.0-alpha.2 | CortexKit official release | CortexKit |
| `ck-claustrum` | 0.1.3 (Arcus `ck-claustrum` 0.1.3-ucs.2 seq 2, published 2026-10-20) | Upstream `claustrum` master built unmodified at `64fe590` of `submodules/claustrum` (branch `sync-upstream-mason` on `rustybret/claustrum`, subc-protocol 0.32.5, subc-transport 0.13, cortexkit-claustrum-client 0.5.0, tests 995 passed), darwin-arm64, with `ck-auth`. Deployed 2026-10-20 to `~/.local/share/cortexkit/bin/ck-claustrum` and codesigned; vault schema 16 preserved (backup: `~/.local/state/claustrum-backup-20261020-162020`). Running binary matches published Arcus package. | Arcus package `ck-claustrum` (us) |
| `ck-mc` | 0.49.16 (upstream tag `v0.49.16`, `c0d5639e1`; Arcus `ck-mc` seq 14 promoted 2026-10-23, verified) | Upstream publishes no current binary (its release is a stale alpha). Built unmodified from the tag on this Mac (local Rust toolchain, MC_BUILD_SHA=c0d5639e13d5bd92db4954ae02d42e55a04b3a1f, stripped, adhoc codesign hardened-runtime). The Arcus envelope for 0.49.16 carries four targets (darwin-arm64 sha256 ef658f7bb4, darwin-x64 sha256 3eef3b3ccf built and signed here; linux-x64 sha256 bb3305f44d, linux-arm64 sha256 6cfe37f8c8 built by Cloudhome Jenkins cortexkit-rust-build #1, both natively verified); the darwin-arm64 binary is byte-identical to the one placed on this Mac. Darwin inputs: private release `ck-mc-v0.49.16-darwin-inputs`. Assembly request sent to Cloudhome 2026-10-21 (message e079dfd8). Arcus catalog promoted 2026-10-23: sequence 14 (ck-mc-0.49.16-14) is `latest`, all four scratch installs verified against the recorded digests (log: .cortexkit/ck-mc-seq14-verification-20261023T221250Z.log), and the stock-daemon sandbox receipt passes (ck-subc 0.20.83, module registers v2, no bind timeouts, exit 0). Placed 2026-10-21 with `place-ck-mc.sh --require-epochs-unchanged` (stores context 95/95, store 63/63, all four epochs unchanged); rollback binaries in `~/.local/share/cortexkit/bin/staging/`. | Arcus package `ck-mc` (Cloudhome builds Linux, we sign Darwin) |
| `ck-entorhinal` | 0.1.20 | Upstream HEAD (`submodules/entorhinal`), unmodified. Nothing is published. | us |
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
