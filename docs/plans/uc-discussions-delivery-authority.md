# uc-discussions as local delivery authority (Option B)

Status: design, approved for work by the operator 2026-11-21. It still needs the design room and Athena review
(which may shrink or kill it) before any code. Requester: uc-studio (PR #58, `docs/reference/shared-thread-gateway-adr.md`
in their repo; that text was not read here). Cloudhome's 7 consumer conditions are the acceptance gates.

## Why

DMs and rooms reach agents through several channels (project mailbox files, `inbox.*`, `rooms.*`, harness-specific
wake paths). Each harness (OpenCode, OMO/Pi/Senpi) ends up with its own notion of "delivered". uc-studio wants one
local record of what was sent to whom, who claimed delivery, and whether it completed, so that a retry, a restart or
a second harness cannot double-deliver or silently drop a turn.

This is the right owner. `ck-uc-discussions` already holds room membership, ordered turns, incarnation fences and
the agent inbox (#7162). Adding the delivery record keeps it in the same transaction as the post it describes. The
alternatives are a second store, which loses atomicity with the post, or the daemon, which is CortexKit's and must not
grow lifecycle state (#6940, #5915).

The boundary stays: the module records intents, claims and completions. It never wakes a session, chooses a model or
runs work. Wake, session choice and delivery mode stay in uc-studio `project_room`.

## What exists, read from source

`modules/uc-discussions` 0.1.2 (`369c3c00`), schema v4.

- `rooms.post` (`src/service/rooms.rs:168`): one `IMMEDIATE` transaction. It allocates `seq = MAX(seq)+1`, which is
  monotonic per room, and checks `incarnation` only when the author is bound. It takes no idempotency key, so a retried
  post is a second post. It does **not** check that the author is a room member: a non-member author gets
  `bound_incarnation = None` and the post is accepted.
- `rooms.bind_member` (`:100`): rejects `incarnation <= current` with `stale_incarnation`. It stores project_id,
  session_id, agent, model, delivery_mode and incarnation.
- `inbox.post` (agent-inbox/v1): dedupes on `UNIQUE(target_agent, idempotency_key)`, with `replace_key` supersede and
  per-item `state`. Each inbox row is already a per-target delivery intent for DMs.
- `peer.acquire_lease`, `renew_lease` and `release_lease`: a generic `leases(resource_id, holder_id, expires_at)` table
  with no generation counter.
- Storage refuses a database whose schema is newer than the binary (`SchemaTooNew`), so downgrading the binary over a
  newer store fails closed.
- Request structs do not use `deny_unknown_fields`, so a 0.1.2 module silently ignores fields it does not know. That
  is the downgrade hazard for gate 3. A new client that sends an idempotency key to an old module gets no dedupe and no
  error, so clients must check capabilities before relying on new semantics.
- No protocol version in the manifest or in requests. No retention: posts, inbox items and leases are kept forever.
  The backup procedure is an ad hoc consistent copy (see `~/.local/state/uc-discussions-backup-*`).

## Architectural ladder

1. **Receipts (gate 3).** Add a table `receipts(principal, operation, key, request_hash, response_json, created_at)`
   with primary key `(principal, operation, key)`. Mutating methods accept an optional `idempotency_key`. A replay with
   the same hash returns the stored response; a different hash returns `idempotency_conflict`. The principal is the
   SubC caller principal, not a client-supplied name. `inbox.post`'s existing key maps onto this table.
2. **Delivery intents, a transactional outbox (gate 2).** Add a table `delivery_intents(intent_id, room_id, post_seq,
   member_id, member_incarnation, state, claim_owner, claim_generation, lease_expires_at, updated_at)`. `rooms.post`
   inserts one row per member other than the author **in the same transaction** as the post. For DMs, the inbox row
   is the intent and gains the same claim columns rather than living in a second table.
3. **Claims with fencing (gate 4).** Add `delivery.claim` (owner, lease_ms, limit), which returns pending or expired
   intents and bumps `claim_generation`. Add `delivery.renew`, `delivery.complete` and `delivery.release`, each taking
   `(intent_id, owner, generation)`. A stale generation or an expired lease returns `stale_claim`. An intent whose
   `member_incarnation` is older than the member's current binding is retargeted, never delivered to the old session.
   Recording only: nothing here starts a process.
4. **Membership enforcement (gate 4).** `rooms.post` requires the author to be a member. `delivery.claim` scopes
   results to the rooms and DMs the caller principal is allowed to deliver for.
5. **Identity (gate 5).** `bind_member` takes `harness` and requires `project_id` to be the entorhinal project id
   (root-commit keyed). The module does not mint identities.
6. **Negotiation (gate 7).** The manifest declares `protocol: "uc-discussions/2"` and a capability list (`receipts`,
   `delivery_intents`, `delivery_claims`). Add a `discussions.capabilities` query. Clients must refuse to use the new
   semantics when the capability is absent, because of the unknown-fields hazard above.
7. **Retention and backup (gate 6).** Configure retention for completed intents and receipts (default 30 days; posts
   are kept). Add `ck discussions backup <path>` via `VACUUM INTO`, plus a documented restore procedure.
8. **Provenance (gate 1).** Each release records the git revision, the binary sha256 and `Cargo.lock` in its Arcus
   descriptor. For ck-subc, cite CortexKit's signed release index only.

## Slicing

Each slice is one release candidate behind its own failing fixtures:

- S1: membership enforcement on `rooms.post`, plus the capabilities query and protocol id. Small, and fixes a real gap.
- S2: the receipts table and idempotent `rooms.post` / `inbox.post`. Schema v5.
- S3: delivery intents written atomically with posts. Schema v6.
- S4: claim, renew, complete and release with generation and lease fencing, and incarnation retargeting.
- S5: retention, the backup and restore commands and their runbook, and identity binding with `harness`.
- S6: a conformance fixture pack for uc-studio and Cloudhome: wire-level request and response JSON pairs, plus
  old/new compatibility pairs.

## Mutation controls

Each acceptance test names the mutation it must catch:

- `rooms_post_skip_membership_check`: a non-member post is accepted. The test expects `not_room_member`.
- `rooms_post_skip_receipt_lookup`: the same key posted twice yields two posts. The test expects one post and an
  identical response.
- `receipt_ignore_request_hash`: same key with a different body returns the old response. The test expects
  `idempotency_conflict`.
- `post_commit_before_intents`: an injected failure after the post insert leaves a post with no intents. The test
  expects neither or both.
- `claim_ignore_lease_expiry`: an expired claim cannot be reclaimed, or a live claim is stolen.
- `complete_skip_generation_check`: a stale owner completes after reclaim. The test expects `stale_claim`.
- `intent_ignore_incarnation`: an intent for incarnation N is claimed for delivery after a rebind to N+1. The test
  expects retargeting.
- `capabilities_omit_receipts`: a client fixture must refuse idempotent mode against a module that lacks the
  capability.

## Still open

- Cross-host delivery is out of scope. This is a local authority on one Mac; SubC federation (callosum) is
  unpublished. Cloudhome harnesses on other hosts cannot use it directly.
- Whether OMO/Pi/Senpi reach it through `@cortexkit/subc-client` (CortexKit's npm package; Cloudhome verified 0.24.4)
  or through `ck-subc-mcp`. That is uc-studio's call; the fixture pack covers the wire either way.
- Linux builds: Arcus verify only runs linux-x64, and there is no Linux pipeline for this module yet.
- Whether `peer.*` (the legacy mailbox-shaped surface) is deprecated once claims exist.
