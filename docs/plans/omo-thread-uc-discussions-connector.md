# OMO thread connector for uc-discussions

Status: design, operator-approved direction 2026-11-24. Contract review deltas received from uc-studio 2026-11-24 (thread doorbell; substantive continuation belongs in a uc-discussions room per decision 3/#6579). Not implemented. Needs design-room / Athena review before code; host sandbox contract remains an external hold until verified.

Decisions recorded 2026-11-24 (operator):
1. Connector owner: **uc-studio** (`project_room` / wake owner), not a new module in this repo.
2. Incarnation source: **room-monotonic per member**. Connector publishes `max(room_current+1, binding_revision)` (or `room_current+1`) on bind/rebind; `rooms.bind_member` already rejects `incarnation <= current`. Binding `revision` is stored as member metadata, not authoritative incarnation. (Refined by D2.)
3. Direct `thread_send` policy: **allowed for doorbells and ad-hoc DMs only; substantive turns go through rooms** (consistent with #6579).
4. Sequencing: **S1+S2 now; manual bind/outbox/ack round-trip against scratch room; S3+S4 before connector ships.** uc-studio connector queues behind their in-flight agent-comms stability work (C1-C4) and behind S4. (D4/D-sequencing note.)

## Why

This project is adding OMO alongside OpenCode. OMO exposes thread tools that already cover the host-side half of the deliberation ladder: durable session addressing, wake/resume, delivery mode (`auto`/`steer`/`follow_up`), idempotency keys, turn fencing (`expected_turn_id`), and a chat-connector binding with a pull outbox. uc-discussions already owns the room-side half: durable room state, ordered posts, membership, incarnation fences, and the planned delivery-authority receipts/claims. Connecting them should reuse those strengths rather than create a third state owner.

The goal is one cross-harness delivery record per room turn, with OMO and OpenCode sessions both addressable as room members, while keeping wake/session choice/delivery execution in the host and room/receipt/claim state in uc-discussions.

## What exists, read from source

### uc-discussions / deliberation plans (this repo)

- `docs/auto-wake-deliberation.md`: `uc-discussions` owns durable rooms, attributable membership, ordered posts, and rejection of posts from stale member incarnations. OpenCode host owns project resolution, process wake, session selection, agent/model selection, and interrupt/enqueue/background delivery. `uc-discussions` does not gain an endpoint registry, process launcher, or outbound host-routing loop. Ladder: Rung 1 session-keyed identity; Rung 3 route-open delivery with failure as liveness signal; Rung 4 wake is policy consultation then launch request; Rung 5 incarnation fence makes wake safe. D-blocked items remain: room fan-out semantics unverified, no wake op signatures in fleet, no registry module locally.
- `docs/plans/uc-discussions-delivery-authority.md`: approved direction, not code. S1 membership enforcement + capabilities; S2 receipts/idempotent mutating methods; S3 delivery intents written atomically with posts; S4 claim/renew/complete/release with generation/lease fencing and incarnation retargeting; S5 retention/backup/identity binding with `harness`; S6 conformance fixture pack. Delivery-authority boundary: module records intents/claims/completions, never wakes/chooses model/runs work. Unknown-fields hazard: 0.1.2 silently ignores unknown fields; clients must check capabilities before relying on new semantics.
- `docs/reference/uc-discussions-participant-policy.md`: colleague declaration is `Council: <name>`. Daemon check is admission filter on recorded metadata, not execution sandbox. Read-only safety is an external host-owner contract / external hold.

### OMO thread tools (verified live in this session, 2026-11-24)

Verified by `tool_schema(...)`, `tool.thread_list()`, and local OMO runtime inspection (`/Users/brethoffman/.omo/binary-runtime/5.1.83/plugin/`):

- `thread_create`: creates fresh session or opens existing by `thread`; supports `alias`, `name`, `cwd`, `worktree`, `fork_from` (currently refused as `fork_unsupported` per schema), `skills`, `model`, `reasoning`, `permission_preset` (`full-access|workspace|accept-edits|auto|read-only|ask`), `prompt`, `delivery`, `idempotency_key`.
- `thread_send`: delivers message to thread by id/alias/unique name/prefix; `delivery auto|steer|follow_up`; `expected_turn_id` required for `steer`; `idempotency_key`; `collapse_key` dedupes/replaces sender's undelivered same-key message; `all_scope` for cross-workspace.
- `thread_read`: transcript/latest output, cursor paging, `wait_ms`, `all_scope`.
- `thread_interrupt`: stops turn and background jobs started by that turn.
- `thread_close`: refuses unread-message thread unless `force`; `graceful`/`wait_ms`.
- `thread_report`: posts `milestone|report|question|completion` to bound chat thread; `question` returns `reply_token` and takes `request_id/request_kind`.
- `thread_outbox` / `thread_outbox_ack` / `thread_answer`: connector pull/ack/reply path.
- `thread_list`: showed live/resumable threads with `thread`, `name`, `status`, `cwd`, `model`, controls, and account-wide scope option. Current session thread: `01a1f5b5-2a2e-706f-830e-c77a118d9445`.

Verified by CLI/help and runtime grep:
- `omo thread bind` exists (`--platform <p> --account <id> --chat <id> [--thread <id>] ... --direction in|out|both --inbound-mode auto|follow_up --events <kind,...> --policy <id> --ttl <seconds>|none --idempotency-key <k>`). `bind|unbind|rebind|bindings` are CLI/gateway operations; agent tools for bind/unbind/rebind/bindings were removed (changelog #13942/#14042/#14141), while `thread_report/outbox/ack/answer` remain.
- Gateway store platform CHECK accepts `discord, telegram, slack, notion, feishu, herdr, custom, whatsapp` (verified in `extensions/gateway-store-worker.mjs` and `runtime/thread-sdk/sdk.js`). **`custom` is available for uc-discussions.**
- Session-gateway store supports namespaced extension state in same transaction as send/bind/ack (changelog #2556 area, not fully audited here).

Unverified / needs proof:
- `permission_preset: "read-only"` actually withholds `bash`, `edit`, `write`, `patch`, shell execution in OMO runtime. This is the host-sandbox external hold.
- Binding creation/TTL/expiry semantics under real uc-discussions-like load.
- Outbox row id stability/format across restarts for use as receipt key.
- Behavior when binding target session is on another device via relay.

### uc-studio contract review inputs (2026-11-24, doorbell thread; claims unverified here)

uc-studio OMO session reported, citing uc-studio source I cannot read across repo boundary: receiver-owned fan-out notes with deterministic ids (`sha256(room:post:member)`, create-if-absent), single dispatch site (`runLocalRoomTurn`) (cited as documented in `packages/omo-opencode/src/features/discussions/AGENTS.md` in uc-studio repo); room-coordinator incarnation derivation (same child session keeps value, new child = prev+1, min 2; convene binds 1); OpenCode room seats host-enforced by per-session tool denylist from `computeRoomMutationDenylist` and council-agent restriction. These are accepted as contract inputs pending owner confirmation and evidence.

## Architectural ladder

### Rung A — Room is authority, OMO is a harness

- `uc-discussions` remains source of truth for room membership, ordered posts, delivery intents, claims, receipts, attribution, and incarnation fences.
- OMO threads are member sessions. A bound OMO member carries `session_id = thread_id`, `harness = "omo"`, `agent = "Council: <name>"` (or other policy-compliant declaration), `delivery_mode` reflecting auto/steer/follow_up mapping, and `incarnation` that is **room-monotonic per member**.
- Binding `revision` is recorded as member metadata (`binding_id`, `revision`) for connector bookkeeping; it is not authoritative incarnation. Connector publishes `max(room_current+1, binding_revision)` or `room_current+1` on bind/rebind.
- OMO thread transcripts are not room records. `ctx_search`/thread history may help an agent, but room ordering/attribution/fencing comes only from uc-discussions.

### Rung B — Connector, not peer-to-peer deliberation

The connector is owned by uc-studio `project_room` / wake owner. It is the only component that translates between uc-discussions delivery state and OMO thread/binding APIs. It must not create a parallel OMO-only delivery path.

**One delivery path (D3):**
- **Pre-S3 (before delivery intents exist):** OMO delivery consumes the same receiver-owned fan-out notes as OpenCode, with deterministic ids as uc-studio reports (`sha256(room:post:member)`, create-if-absent), as a per-member transport behind the coordinator. Note id is the idempotency key.
- **S3+S4:** delivery intents replace fan-out notes for both harnesses at once. Claims/generation/lease fencing apply uniformly.

**Room -> OMO (inbound to agent):**
1. Connector acquires work: pre-S3, a receiver-owned fan-out note; S3+, `delivery.claim` an intent for an OMO-bound member.
2. It resolves binding by `(platform=custom, account=uc-discussions, chat=<room_id>, thread=<member_id>)` or stored binding id.
3. It sends via `omo thread send --binding <binding_id> <text> --idempotency-key <intent_id_or_note_id>:<target_incarnation> --mode follow_up|auto` (mode selected by policy; `steer` only when turn fence is known/safe). **Idempotency key must not include claim generation** (D1): reclaim after send-accepted/complete-lost must dedupe, while generation fences only completion.
4. On accepted delivery, connector completes the work item (pre-S3 note completion path as defined by uc-studio; S3+ `delivery.complete` with same claim generation). On claim loss/expiry, no complete; work becomes reclaimable per rules.

**OMO -> Room (outbound from agent):**
1. Agent calls `thread_report(kind=report|question|completion|milestone, ...)`.
2. Connector drains `omo thread outbox <binding_id> --after <cursor>`.
3. For report/completion/question-as-turn, connector posts to `rooms.post` / appropriate uc-discussions op with idempotency key `binding_id:revision:row_seq` (or equivalent stable composite) until raw outbox row id stability across restart is proven (D7). It must check uc-discussions capabilities before relying on receipts.
4. Connector acks outbox row only after uc-discussions durably accepts or returns an idempotent replay.
5. For `question`, connector relays room answer back through `thread_answer`.

### Rung C — Fencing

- **Delivery-intent fence (uc-discussions S4):** claim generation/lease is authoritative for "who may complete this intent"; generation is not part of send idempotency key (D1).
- **Member incarnation fence:** room-monotonic per member (D2). Rebind/new session publishes `max(room_current+1, binding_revision)` or `room_current+1`; daemon rejects non-increasing incarnation. Intents/notes targeted at old incarnation are retargeted/not delivered to stale session.
- **Turn fence (OMO local):** `expected_turn_id` used only when connector has fresh turn id and policy selects steer; otherwise use follow_up/auto to avoid spurious turn conflicts.

### Rung D — Read-only Council seats

For Council colleagues, host creates/binds threads with `permission_preset:"read-only"`, Council-compliant `agent` declaration, and least-privilege skills. This is necessary but not sufficient: the daemon guard (participant policy) is admission metadata; actual filesystem/tool sandbox is host-enforced and externally held until proven by forbidden-write tests. OMO read-only seats need forbidden-write proof equivalent to OpenCode's host-enforced per-session tool denylist / council-agent restriction before joining advisory-safe rooms (D6).

## Slicing

- **S1+S2 (now, uc-discussions):** membership enforcement, capabilities query/protocol id, receipts/idempotent `rooms.post`/`inbox.post`. No connector dependency.
- **Manual proof (before S3/S4 connector ship):** against a scratch room/binding, prove bind -> send -> report -> outbox -> ack -> answer loop with idempotent retries. Record evidence. Include reclaim-after-send-accepted/complete-lost case (D1).
- **S3+S4 (before connector ships):** atomic delivery intents, claim/renew/complete/release generation+lease fencing, incarnation retargeting. Intents replace fan-out notes for both harnesses at once (D3).
- **S5:** retention, backup/restore, identity binding with `harness="omo"`, and member metadata for binding id/revision.
- **S6:** conformance fixture pack including OMO connector wire pairs and old/new capability pairs.
- **Connector implementation (uc-studio):** after S4 semantics are available and capability-gated; OpenCode path remains supported. uc-studio connector work queues behind uc-studio in-flight agent-comms stability work (C1-C4) and behind S4.

## Mutation controls / verification

Mutation controls from `uc-discussions-delivery-authority.md` remain required (membership skip, receipt lookup skip, hash ignore, post-before-intents, lease expiry ignore, generation skip, incarnation ignore, capabilities omit).

Connector-specific controls to add:
- **`ack_before_room_post_durable`**: crash/retry after ack but before durable room post must not lose the report; test expects report present once after replay, no lost row.
- **`complete_without_claim_generation`**: stale worker completes after reclaim; test expects `stale_claim` / no completion.
- **`drop_idempotency_key_on_send`**: retried intent/note send injects duplicate agent turn; test expects single delivery effect.
- **`send_key_includes_claim_generation`** (D1): reclaim after send-accepted/complete-lost with generation in key causes duplicate delivery; test expects single delivery effect.
- **`ignore_room_monotonic_incarnation`** (D2): rebind/move publishes incarnation <= room current or old intent delivered to stale session; test expects daemon rejection or retarget/refusal.
- **`treat_read_only_prompt_as_sandbox`** (D6): agent under read-only preset can write file; forbidden-write test must fail closed before claiming read-only safety.
- **`parallel_omo_delivery_path`** (D3): pre-S3 connector bypasses receiver-owned fan-out notes / post-S3 bypasses intents; test expects single authority path.

All verification claims for connector behavior must be live end-to-end against real OMO and real uc-discussions, with evidence files. Mock/unit tests are not sufficient proof.

## Still open

1. **Read-only sandbox proof (external hold, D6):** confirm `permission_preset:"read-only"` denies mutating tools in the actual OMO runtime used by fleet; add forbidden-write test equivalent in force to OpenCode per-session tool denylist / council-agent restriction.
2. **Binding identity details (D7):** confirm stable binding id/revision/outbox row semantics across restart/rebind/expiry; confirm `--ttl none` is honored for long-lived room seats. Outbound key uses `binding_id:revision:row_seq` until raw row id stability proven.
3. **Cross-device OMO sessions:** relay makes threads account-wide; confirm binding/outbox semantics when session moves devices or is addressed as `<device>/<thread>`.
4. **Steer vs follow_up policy:** default to follow_up for room turns unless fresh `expected_turn_id` known; define when auto-steer is safe.
5. **Room fan-out semantics remain unverified** (carried from auto-wake design); D3 accepts uc-studio's stated fan-out invariant as input pending evidence.
6. **uc-discussions protocol/capability negotiation** must land before connector relies on receipts/claims due unknown-fields hazard; uc-studio adapter currently does not call `discussions.capabilities` (D5).
7. **Connector placement and dependency (D4, uc-studio owner):** host-resident (needs live OpenCode host per receiving project) vs standalone uc-studio worker draining OMO outbox. OMO CLI dependency and OpenCode-only scope need owner confirmation.
8. **Ownership boundary:** connector code lives in uc-studio; this repo provides uc-discussions authority, fixtures, and contract docs only. No direct edits across repos.
9. **Room for substantive continuation:** per decision 3/#6579, contract deltas move to a uc-discussions room; thread is doorbell only. uc-studio will open room and invite subconscious once their owner rules on D4 (connector ownership/scope/placement); expect room id on doorbell thread.

## Connector contract (draft for uc-studio, revised by D1-D7)

This is the minimum contract uc-studio's connector should implement:

- **Binding key:** `platform=custom`, `account=uc-discussions`, `chat=<room_id>`, `thread=<member_id>`; store binding id and revision as metadata.
- **Create/bind seat:** create or reopen OMO thread; bind with `--direction both --inbound-mode follow_up --ttl none`; set `permission_preset read-only` for Council seats. Publish room-monotonic incarnation (`max(room_current+1, binding_revision)` or `room_current+1`).
- **Inbound delivery (one path):** pre-S3 consume receiver-owned fan-out notes (deterministic id, note id as idempotency); S3+ claim delivery intent. Send via `omo thread send --binding ... --idempotency-key <intent_or_note_id>:<target_incarnation> --mode follow_up`. Do not include claim generation in send key. Complete only with matching claim generation (S3+).
- **Outbound drain:** poll/receive outbox -> post to uc-discussions with idempotency key `binding_id:revision:row_seq` (until row id stability proven) -> ack outbox -> resolve question via `thread_answer`.
- **Incarnation:** on rebind/new session/member harness move, publish room-monotonic incarnation; stale intents/notes retarget. Binding revision kept as metadata.
- **Capability gate:** refuse to use receipts/claims/delivery intents unless uc-discussions advertises them.
- **Evidence:** live E2E run with real uc-discussions store/module, real OMO thread, forced retry, forced reclaim-after-send-accepted/complete-lost, forced stale claim, forced rebind/incarnation regression; record artifacts.
