# Changelog

Arcus package `uc-discussions` (binary `ck-uc-discussions`). Versions are display-only; the Arcus sequence orders releases.

## 0.1.2 (unreleased)

- Agent Inbox: native implementation of the `agent-inbox/v1` wire specification (`cortexkit-role-agent-inbox` 0.1.0) alongside `peer.*`:
  - `inbox.post` with envelope neutralization (`neutralize`, U+2039 swap), 16 KiB size cap enforcement, `(target_agent, idempotency_key)` deduplication replay, and `replace_key` supersession.
  - `inbox.withdraw` for retiring pending notices.
  - `inbox.list` and `inbox.read` for reading neutralized notice items and metadata.
  - Schema migration 003 adding `agent_inbox_items` table with unique idempotency and lookup indexes.
- Extensible CLI domain: added `--ck-domain` headline handshake and operator CLI verbs (`status`, `list`, `room <id>`, `council <id>`, `post <id> <content>`), dynamically mounted by the stock `ck` operator dispatcher under `ck discussions`.
- Submodules: added `submodules/` tracking upstream CortexKit source-built repositories (`entorhinal`, `claustrum`, `fusiform`, `astrocyte`).

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
