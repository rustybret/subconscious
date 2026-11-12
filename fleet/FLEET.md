# Fleet module sources

What runs under the local `ck-subc`, where each binary comes from, and who builds it. Update this
file in the same commit that changes a module's source. Verified 2026-11-12.

## Setup-managed (CortexKit official releases)

Installed and upgraded by `ck setup <component>` / `ck upgrade` from the signed index
`https://cortexkit.io/releases/v1/index.json`; recorded in `~/.local/share/cortexkit/installer-manifest.json`.
Never hand-place these: `ck setup` refuses binaries it did not install.

| Module | Version | Notes |
| --- | --- | --- |
| `ck-subc`, `ck`, `ck-subc-mcp` | 0.21.3 (`subc-core-v0.21.3`; `ck-subc-mcp` reports 0.2.2) | Cut over 2026-11-11; `ck` re-placed by `ck upgrade` 2026-11-12 so setup records it. `ck upgrade --check` reports everything up to date. |
| `ck-claustrum`, `ck-auth` | 0.3.0 (`119f56b2`) | Installed 2026-11-12 with `ck setup claustrum --key-path ~/.config/cortexkit/claustrum.key` (key file custody unchanged). Vault schema 18 -> 32; 0.3.0 mints `kem:secret-deposit:1`. Audit chain intact, stranded 0. Pre-upgrade copy: `~/.local/state/setup-migration-20261111-203127/claustrum-pre-0.3.0/`. |
| `ck-insula` | 0.2.1 (`135b0b374e9c`) | Replaced our 0.1.1 source build 2026-11-12. Reads logins from the vault: grants `reserved:insula` read on categories `llm-provider` and `browser-session`. |
| `ck-aft` | 0.64.0 | Plugin `@cortexkit/aft-opencode@0.64.0`. Runs at the daemon's default 12 s route.bind relay timeout. |
| `ck-synapse` (+ `opctl`, workers) | 0.1.0-alpha.2 | Reinstalled from the release 2026-11-12 (the previous binaries were not the release bytes). |

## Built by us (no upstream release)

| Module | Version | Source |
| --- | --- | --- |
| `ck-entorhinal` | 0.1.46 (`84677e1`) | `submodules/entorhinal`, unmodified upstream. The submodule pointer is ahead of the deployed build. |
| `ck-fusiform`, `ck-models` | 0.1.0 | `submodules/fusiform` (`ck models` CLI). |
| `ck-astrocyte` | 0.1.0 | `submodules/astrocyte` (AI usage and spend ledger). |
| `ck-uc-discussions` | 0.1.2 (Arcus package `uc-discussions` seq 3) | `modules/uc-discussions/`, at `~/.local/bin/ck-uc-discussions`. |

## Retired

- `ck-mc` (magic-context Rust daemon): retired 2026-11-12 (operator decision) until upstream publishes real release
  binaries. The plugin runs `transform_mode: "ts"` because rust mode crashed and restarted; in ts mode nothing
  opened a route to the module (zero `route.open` to `magic-context` 2026-11-05..12). Entry removed from `subc.jsonc`;
  0.49.52 binary and config copy in `~/.local/state/ck-mc-retired-20261111-214614/`. `orw-probe.sh` reports
  `ck-mc: SKIPPED`. To return: rebuild from the upstream tag or install a published release, restore the entry.
- `lore-thalamus`: retired 2026-11-12 (operator-approved); removed from `subc.jsonc`, binary archived in
  `~/.local/state/setup-migration-20261111-203127/archive/`. That completes Lore's removal from the fleet.
- `lore-broca`, `lore-wake`, `lore-mcp-gateway`: retired 2026-10-17; archived in `~/.local/state/lore-retired-20261017-073526/`.

## Daemon configuration

Stock since 2026-11-12: the launchd plist is setup's own definition (no `XDG_RUNTIME_DIR`), so the daemon writes its
connection file to `$TMPDIR/subc-<uid>.connection.json`. Clients auto-discover it; nothing may pin
`~/.local/share/cortexkit/run/subc-connection.json` (ck on macOS checks that path first, so a stale file there
breaks discovery). No module carries `reserved` except claustrum, uc-discussions and entorhinal, and no
module overrides `route_bind_relay_timeout_ms`.
