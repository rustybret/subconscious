# Changelog

Arcus package `uc-discussions` (binary `ck-uc-discussions`). Versions are display-only; the Arcus sequence orders releases.

## 0.1.1 (Arcus sequence 3)

First release since the 0.1.0 package (sequence 2, packed 2026-09-20). Everything below landed after that pack.

- Rooms: `rooms.convene`; member session bindings with agent, model and delivery mode; monotonic incarnation fences
  (`stale_incarnation` rejection); schema v2.
- Closed rooms are frozen: object, revise, grant_stage, vote and repeated close are rejected, and the first close
  outcome, dissent and timestamp are immutable.
- Post admission, the incarnation fence, reply-target validation and sequence allocation share one SQLite transaction.
- Council: `council.get` read-back; idempotent `council.stage`; `council.reconcile` checks the persisted roster, not
  the caller's list; `council.evaluate` rejects non-roster members and cannot overwrite a terminal state.
- Snapshot export and import keep the schema v2 member identity fields.
- CLI: `--version` and `--help` exit 0 before any storage or subc initialisation; storage failure reports `failing`.
- Depends on published `subc-*` and `cortexkit-store-types` crates instead of workspace paths.

## 0.1.0 (Arcus sequence 2)

Initial discussions daemon: rooms, posts, objections, revisions, polls, stage grants, councils, peer messaging.
