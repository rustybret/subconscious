# Blessed set and `ck setup` for the UltraCreative fleet

Verified 2026-10-17 on the reference workstation. Source of record for module provenance is
`fleet/FLEET.md`; this document is what a host installer (uc-studio `ucs-setup`) should encode.

## Principle

Run CortexKit's published releases wherever a usable one exists. Build or package ourselves only what
we write, or what upstream does not publish in a form the stock daemon accepts. Never build the subc
daemon, `ck`, or any `subc-*` library locally.

## What `ck setup` does and does not do

`ck setup [aft|mc|insula|claustrum|synapse] [--with ...] [--dry-run] [--verbose]` installs the core and
named components from `https://cortexkit.io/releases/v1/index.json`, sha256-verified. Observed:

- Bare `ck setup` installs immediately; it does not ask. Always run `--dry-run` first.
- It installs core (`ck`, `ck-subc`, `ck-subc-mcp`) into `~/.local/share/cortexkit/bin/`, adopts a
  byte-identical `ck-subc`, and places `ck-subc-mcp`.
- It refuses to rewrite a launchd definition that differs from its template and never stops a running
  daemon. Stop with `launchctl bootout gui/$UID/cortexkit.subc`, re-run, then load.
- Its launchd template (`Label`, `ProgramArguments`, `RunAtLoad`, `ExitTimeOut`) carries no
  `EnvironmentVariables`. Our daemon needs `XDG_RUNTIME_DIR=$HOME/.local/share/cortexkit/run` so the
  connection file does not land in the macOS temp directory, which the OS purges. Add it after
  `ck setup` writes the plist, and verify the connection file path. Whether 0.20.83 still falls back
  to the temp directory without it is not verified here; test in a sandbox before relying on the default.
- `ck upgrade --check` reports available updates; `ck upgrade --dry-run` prints the plan.

## Blessed set

Use `ck setup` for these (official, signed or digest-verified):

| Component | Version | Notes |
| --- | --- | --- |
| `ck`, `ck-subc` | 0.20.83 (`subc-core-v0.20.83`) | The oldest daemon every module must interoperate with. |
| `ck-subc-mcp` | 0.1.34 (core 0.20.83) | stdio MCP shim for external clients, not a supervised module. |
| `ck-insula` | 0.1.1 | `ck setup insula` |
| `ck-synapse` | 0.1.0-alpha.2 | `ck setup synapse` |

Do NOT use `ck setup` for these; they come from elsewhere:

| Component | Version | Why not `ck setup` | Source |
| --- | --- | --- | --- |
| `aft` | fork 0.58.2 (seq 18) live; 0.60.0 (seq 20, tag `v0.60.0_seq20`) verified, not yet in the catalog | Upstream `aft` lacks our SubC federation, LSP and GDScript changes. `ck setup aft` would replace the fork. | Arcus package `opencode-aft` |
| `ck-mc` | 0.49.10 from upstream tag `v0.49.10` (plugin and daemon are a pair; built unmodified); Arcus package `ck-mc` seq 11, four targets | The `mc` entry in the index is a stale alpha (0.1.0). | Built from upstream tag; Cloudhome is the target builder |
| `ck-claustrum` | upstream `master` merge, reports 0.1.3 (module version; NOT the `@cortexkit/claustrum-client` npm library, which is 0.9.x/0.10.x and a different artifact). Arcus package `ck-claustrum` 0.1.3-ucs.1 seq 1 is published (darwin-arm64) | The published 0.1.3 predates the launch-nonce fix and is rejected by the daemon (`reserved_module`). `ck setup claustrum` would install a module that crash-loops. Wait for a fixed upstream release. | Source build until then |
| `ck-entorhinal` | 0.1.20 (upstream HEAD, unmodified) | Nothing is published. | Source build |
| `ck-uc-discussions` | 0.1.1 (Arcus package `uc-discussions`, seq 3, published) | Ours. | Arcus package `uc-discussions` |
| OpenCode plugin `@cortexkit/opencode-magic-context` | pin an exact version (the workstation is pinned 0.49.10, which is also npm latest) | Consumed from npm, not `ck setup`. | npm |

Exclude: every `lore-*` module. Lore is deprecated. `lore-broca`, `lore-wake` and `lore-mcp-gateway` are
retired; `lore-thalamus` is still running on the reference workstation but must not be in the blessed set.

## Gate before a component enters the blessed set

1. Run it against the stock daemon in a sandbox: `fleet/isolated-daemon.sh --module NAME=PATH --reserved NAME`.
   It must register at protocol v2, report health `ok`, show zero `route.bind` timeouts or breaker trips,
   and still be running at the end. Registration alone is not a pass; a module that registers then exits
   is a crash loop. Stage the binary on the internal disk, not an external volume.
2. Fleet wire-pin compatibility is checked against the stock daemon, not against crates.io: the newest
   published `ck-subc` binary (core 0.20.83: daemon 0.34.5, protocol 0.31) lags the published libraries
   (daemon 0.36.6, protocol 0.32.4). A module built on newer libraries still registered at v2 against it.
3. A vault-owning module (claustrum) must be tried against a copy of the live vault first: the current
   build migrates the schema forward (13 to 16). Back up with `sqlite3 store.db ".backup ..."` first.

## Drift detection

`fleet/orw-probe.sh` compares the pinned plugin and the installed `ck-mc` against upstream. Cloudhome owns
the build and Arcus packaging path; workstations should not compile.
