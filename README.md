# subconscious (UltraCreative subc modules)

This repository holds the subc modules and tooling that UltraCreative writes itself. They run
under the **stock, published `ck-subc` daemon** next to CortexKit's own modules. Nothing here
builds the daemon, the `ck` CLI or any `subc-*` library: those come from CortexKit's official
installer (`curl -fsSL https://cortexkit.io/install | bash`) and from crates.io.

Upstream is [cortexkit/subconscious](https://github.com/cortexkit/subconscious). This repo used to be
a fork of it carrying two daemon patches; both were retired in favour of configuration (see
`docs/reference/fleet-custom-vs-published-matrix.md`).

## Layout

| Path | What it is |
| --- | --- |
| `modules/uc-discussions/` | `ck-uc-discussions`: durable rooms, turns, member session bindings, monotonic incarnation fences and council rosters over subc channel 0. Depends only on published crates. |
| `packaging/arcus/` | Arcus package descriptors and `bootstrap.sh`, which installs the publisher toolchain. |
| `packaging/scripts/` | Pack, publish, sign and validate scripts for our own modules (lifecycle scripts are symlinks into the toolchain). |
| `fleet/` | `FLEET.md` (which module comes from where), `orw-probe.sh` (plugin and daemon drift check) with tests, `watch-room.sh`. |
| `docs/` | Design notes, evidence, the plan, references and the `uc-discussions` runbook. |

A new module is a new `modules/<name>/` crate, a descriptor in `packaging/arcus/`, an entry in
`~/.config/cortexkit/subc.jsonc`, and a row in `fleet/FLEET.md`.

## Build and test

```sh
cargo test --locked            # uc-discussions unit and integration suites
cargo build --release --locked # target/release/ck-uc-discussions
python3 fleet/tests/test_orw_probe.py
```

If `~/.cargo/config.toml` sets an sccache `rustc-wrapper` and builds hang, disable it with
`cargo --config 'build.rustc-wrapper=""' ...`.

## Packaging

```sh
sh packaging/arcus/bootstrap.sh   # once: installs/links the Arcus publisher toolchain
bun run pack:arcus                # dist/<sequence>/<package>/<version>/
```

The release flow, sequence rules and tag naming are in `docs/reference/arcus-packaging-and-publishing.md`.

## Identity and history

Magic-context identifies this project by its **root commit** (`git rev-list --max-parents=0 HEAD`,
`18e126e4...`). Renaming the GitHub repository or moving the directory is safe. Rewriting history or
re-creating the repository is not: it would orphan every stored memory and rule.
