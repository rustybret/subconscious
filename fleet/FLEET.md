# Fleet module sources

What runs under the local `ck-subc`, where each binary comes from, and who builds it. Update this
file in the same commit that changes a module's source. Verified 2026-11-22 (`ck module list`, `ck catalog`,
`ck upgrade --check`).

## Setup-managed (CortexKit official releases)

Installed and upgraded by `ck setup <component>` / `ck upgrade` from the signed index
`https://cortexkit.io/releases/v1/index.json`; recorded in `~/.local/share/cortexkit/installer-manifest.json`.
Never hand-place these: `ck setup` refuses binaries it did not install.

| Module | Version | Notes |
| --- | --- | --- |
| `ck-subc`, `ck`, `ck-subc-mcp` | 0.21.3 (`subc-core-v0.21.3`; `ck-subc-mcp` reports 0.2.2) | Cut over 2026-11-11; `ck` re-placed by `ck upgrade` 2026-11-12 so setup records it. `ck upgrade --check` reports everything up to date. |
| `ck-claustrum`, `ck-auth` | 0.3.0 (`119f56b2`) | Installed 2026-11-12 with `ck setup claustrum --key-path ~/.config/cortexkit/claustrum.key` (key file custody unchanged). Vault schema 18 -> 32; 0.3.0 mints `kem:secret-deposit:1`. Audit chain intact, stranded 0. Pre-upgrade copy: `~/.local/state/setup-migration-20261111-203127/claustrum-pre-0.3.0/`. |
| `ck-insula` | 0.4.4 | Replaced our 0.1.1 source build 2026-11-12; upgraded 0.3.1 -> 0.4.0 by `ck upgrade` 2026-11-22, then 0.4.0 -> 0.4.4 on 2026-11-27. Reads logins from the vault: grants `reserved:insula` read on categories `llm-provider` and `browser-session`. |
| `ck-aft` | 0.68.1 (module binary); plugin `@cortexkit/aft-opencode@0.67.1` | Plugin spec corrected from `cortexkit/aft@0.66.0` on 2026-11-21. Module upgraded 0.66.0 -> 0.67.1 -> 0.68.1 by `ck upgrade` through 2026-11-27; `ck upgrade` does not rewrite the npm plugin spec. Runs at the daemon's default 12 s route.bind relay timeout. |
| `ck-synapse` (+ `opctl`, workers) | 0.1.0-alpha.2 | Reinstalled from the release 2026-11-12 (the previous binaries were not the release bytes). |

## Built by us (no upstream release)

| Module | Version | Source |
| --- | --- | --- |
| `ck-entorhinal` | 0.1.46 (`84677e1`) | `submodules/entorhinal`, unmodified upstream. The submodule pointer is ahead of the deployed build. |
| `ck-fusiform`, `ck-models` | 0.1.0 | `submodules/fusiform` (`ck models` CLI). |
| `ck-astrocyte` | 0.1.0 | `submodules/astrocyte` (AI usage and spend ledger). |
| `ck-uc-discussions` | source and deployed 0.2.0-alpha.1 (Arcus-published 0.1.2, pkg `uc-discussions` seq 3) | `modules/uc-discussions/`, deployed at `~/.local/bin/ck-uc-discussions` (sha256 `b2fd43db...`) since 2026-11-22. S1 adds the author-membership check and `discussions.capabilities`; built locally from `c5a329d8` and not yet re-packaged for Arcus. |

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

Since 2026-11-27: 314track added `XDG_RUNTIME_DIR=/Users/brethoffman/.local/share/cortexkit/run` to
`~/Library/LaunchAgents/cortexkit.subc.plist` after the daemon lost its `$TMPDIR` connection file. The daemon now
writes `~/.local/share/cortexkit/run/subc-connection.json` (regular file, mode 0600); clients auto-discover it,
and `~/.zshrc` exports `CLAUSTRUM_SUBC_CONNECTION` and `OPENCODE_ANTHROPIC_AUTH_CLAUSTRUM_CONNECTION_FILE`
point there. This plist change deviates from setup's stock definition and is under review in uc-discussions
room `room-b8adf64c-bed0-4aa6-972c-3cd31d2f243a`. A stale file at the old `$TMPDIR` path would still break
discovery, so do not recreate one. No module carries `reserved` except claustrum, uc-discussions and
entorhinal, and no module overrides `route_bind_relay_timeout_ms`.
uc-discussions and entorhinal, and no module overrides `route_bind_relay_timeout_ms`.
