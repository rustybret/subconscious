# Contributing

This is UltraCreative's own repository of subc modules and packaging; it is not a CortexKit
contribution target. Changes to the daemon, the `ck` CLI or any `subc-*` crate belong in
[cortexkit/subconscious](https://github.com/cortexkit/subconscious).

- Keep `modules/*` on published crates only (no `path` dependencies outside this repo).
- A module change updates its `CHANGELOG`, its version, and `fleet/FLEET.md` when its source changes.
- Run `cargo test --locked` and `python3 fleet/tests/test_orw_probe.py` before pushing.
- Never rewrite history on `master`: project memory is keyed on the root commit.
- Keep internal working notes under `.cortexkit/` or `.omo/` (git ignores both).
- Do not read or modify the `arcus` repository directly; coordinate with it by project message.
