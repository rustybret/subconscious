# Entorhinal adoption guide for uc-studio

This is the implementation and cutover guide for replacing uc-studio's local project registry (`~/.ucs/project-registry.json`), heartbeat files (`~/.omo/presence/*.json`) and file-based routing with the local `ck-entorhinal` service. The target is Entorhinal **0.1.46**, serving `project-identity/v1` and `agent-identity/v1` through SubC Channel 0. Entorhinal owns durable project identity and answers session liveness; it does not own mailbox claims, room state, or message delivery.

The JSON below is the ManagementSurface request body (`{method, params}`) and the module's JSON result body. SubC's outer Channel 0 framing, route opening, and response correlation are handled by the TypeScript SubC client, not by these JSON snippets. The Entorhinal source in this repository defines no TypeScript client method signatures; the `rpc` abstraction below is intentionally a thin pseudocode adapter over the client version uc-studio installs. Do not copy its illustrative method name as a claim about the SDK API.

## Ownership mapping and boundaries

| Legacy uc-studio state/behavior | New authority | Migration consequence |
|---|---|---|
| `~/.ucs/project-registry.json` path-to-project table | `projects.resolve` for a path; `projects.resolve_project_id` for a known ID; `projects.enumerate` (CLI equivalent `projects.list`) for the registry view | Persist/use Entorhinal's `pj-...` IDs. `resolve` may return an implicit ID for an unregistered absolute path; do not treat that as proof of a registered project. |
| `~/.omo/presence/*.json` heartbeat/state files | `agent.session_liveness` reads the host-fed in-memory mirror | Delete file polling/writes only after every liveness producer is feeding the host API and every router uses the query. No stale disk heartbeat files are left behind. |
| Route to an agent based on presence-file freshness | Resolve the agent's registered session reference and query `agent.session_liveness` | `active` permits attempting direct delivery; `gone` means known ended; `unknown` is not evidence of absence and must be treated as busy/uncertain. Liveness is advisory, not a delivery guarantee. |
| Primary mailbox claim/lease | Receiver-local uc-studio / inbox storage | Keep it local. A lease arbitrates which receiver owns processing; Entorhinal reports a host's view of session state and is neither a distributed lock nor a mailbox transaction coordinator. |
| Room membership incarnation / stale-post fencing | `ck-uc-discussions` | Keep monotonic room incarnation checks and reject stale posts there. Liveness can change or become unknown after restart and cannot fence an already-issued room write. |

Entorhinal's `agent.session_liveness` answers from its last host feed; it does not poll processes. `unknown` must never be interpreted as `gone`. Unknown/gone agents can be woken using the existing uc-studio host wake mechanism, then their new session binding registered. Room creation, durable turns and inbox doorbells remain `ck-uc-discussions`/`project_message` responsibilities.

## Channel 0 connection and routing

1. Locate the daemon's connection descriptor at `~/.local/share/cortexkit/run/subc-connection.json` (or the configured SubC connection path when uc-studio already supports one). Give that descriptor to the supported TypeScript SubC client. Let the client perform the daemon's authenticated connection/HELLO exchange and establish Channel 0; do not open the socket or manufacture management frames yourself.
2. Open a ManagementSurface route to module `entorhinal` through the client, then send the `{method, params}` bodies shown below. Entorhinal's documented module surface routes project and agent methods on this surface. Successful responses have a `result` object; refusals carry structured `code`, `message`, and `detail` (depending on client API, surfaced as a result error rather than a normal result).
3. Keep connection, route, and request correlation lifecycle in the SubC client. Do not hold a uc-studio global project/presence mutex while awaiting a Channel 0 response. Project reads use Entorhinal's read path and do not join its serialized shared-write queue; a blocked network call under an application-wide lock can still block uc-studio itself. Snapshot local state under a short lock, release it, perform RPC, then apply the response under a short lock if still relevant.
4. Project queries are open to every caller. Mutations have separate admission rules: project `register`/`attach_root` are Direct or executive writes and may prompt for operator confirmation. `agent.register_session` is restricted to the daemon-resolved sole provider of `agent-orchestrator/v1` or `agent-host/v1`, not an arbitrary local Direct client. Thus uc-studio's ordinary local process may query liveness but cannot publish the host feed or register bindings unless it is operating as the admitted capability provider.

The source-side pseudocode boundary should be implemented against the actual TypeScript client's documented route API:

```ts
// PSEUDOCODE: replace only this transport seam with the installed SubC TS SDK API.
// Connection descriptor: ~/.local/share/cortexkit/run/subc-connection.json
interface ManagementRoute {
  request(body: { method: string; params: unknown }): Promise<unknown>;
  close(): Promise<void>;
}
interface SubcClient {
  openManagementSurface(moduleId: "entorhinal"): Promise<ManagementRoute>;
}

class EntorhinalPresenceAdapter {
  private constructor(private readonly route: ManagementRoute) {}

  static async connect(client: SubcClient): Promise<EntorhinalPresenceAdapter> {
    return new EntorhinalPresenceAdapter(await client.openManagementSurface("entorhinal"));
  }

  private async call<T>(method: string, params: unknown): Promise<T> {
    const wire = await this.route.request({ method, params }) as { result: T };
    return wire.result;
  }

  close(): Promise<void> { return this.route.close(); }
}
```

The adapter should validate successful result shapes and surface structured transport/refusal errors; the abbreviated cast above is illustrative, not a recommendation to trust unvalidated JSON. Keep the route open for the adapter lifetime if the SDK supports it, and reconnect/reopen after connection loss.

## Project identity RPCs

### Resolve an absolute path

```json
{"method":"resolve","params":{"canonicalRoot":"/absolute/path/to/project"}}
```

The required field is `canonicalRoot` (alias `canonical_root` is accepted by the module). The input may be any absolute path, not only a registered root; Entorhinal canonicalizes it. A success result includes `projectId`, `via`, `gone`, `generation`, and `incarnation`, plus metadata such as `canonicalRoot`, `workspaceId`, `projectName`, `matchedRoot`, `rootRecords` when applicable. `via` distinguishes exact root, contained walk, worktree and implicit resolution. For a file inside a registered root, `via: "walk"` can identify the project. An implicit result may contain `pj-implicit1-...`; it is not equivalent to a project registration. Check `gone` and use `via`/root metadata rather than blindly assuming every returned ID is a registered project.

```ts
type ResolveResult = {
  projectId: string;
  via: string;
  gone: boolean;
  generation: number;
  incarnation: string;
  canonicalRoot: string;
  workspaceId: string | null;
  projectName: string | null;
};

async resolveProjectPath(absolutePath: string): Promise<ResolveResult> {
  return this.call("resolve", { canonicalRoot: absolutePath });
}
```

Use `path.resolve`/realpath in uc-studio as appropriate to its filesystem semantics, but do not assume its spelling is already canonical; Entorhinal reports the canonical query path. The stable identity is the returned project ID, not the path string.

### Resolve by durable project ID

```json
{"method":"resolve_project_id","params":{"projectId":"pj-example"}}
```

Successful result carries `projectId`, `via`, `gone`, `generation`, and `incarnation`; current IDs and aliases resolve, while an unknown ID can return a successful `via: "gone", gone: true` result. This lookup is useful to refresh metadata for IDs already persisted by uc-studio. Do not encode an error assumption that every unknown identifier is a transport refusal.

### Enumerate or list

```json
{"method":"enumerate","params":{}}
```

Optionally filter using `{"workspaceId":"ws-..."}`. Success contains `workspaces`, `projects`, `generation`, and `incarnation`. Project entries include IDs, names, roots, implicit/ref-kind and available root records; some optional fields are absent rather than null. The `ck projects list` CLI is the operator-facing list operation; for an adapter use the documented ManagementSurface method `enumerate` (not a guessed `projects.list` wire method).

### Register and attach roots (writes)

`projects.register` is a mutation that mints a durable ID. The `RegisterRequest` source type (`crates/entorhinal-core/src/mutations.rs`) uses camelCase JSON fields: `name` is required; optional/nullable-by-omission values include `label`, `projectId`, `domain`, `workspaceId`, `roots` (default empty), `derivedRootParents` (default empty), `requestKey`, and `actor`. Use idempotency `requestKey` for retries. Example shape:

```json
{"method":"register","params":{"name":"uc-studio project","roots":["/absolute/path/to/project"],"requestKey":"uc-studio-register-<stable-operation-id>"}}
```

`projects.attach_root` takes the `AttachRootRequest` source type (`crates/entorhinal-core/src/root_keys.rs`): `path` required; optional `projectId`, `label`, `actor`, and `requestKey`.

```json
{"method":"attach_root","params":{"path":"/absolute/path/to/checkout","projectId":"pj-existing","requestKey":"uc-studio-attach-<stable-operation-id>"}}
```

Registration and root attachment are not shadow-read operations. Route them only through the authorized write path and handle operator-confirmation refusal. Attach starts unapproved; do not treat attachment as approval for execution. `attach_root` associates another local checkout with existing identity; it is not a way to mint a new ID. In `0.1.46`, consult the module surface and request types before depending on optional root behavior; no extra fields are implied here.

## Agent sessions and liveness

### Register a session binding

`agent.register_session` binds `(ref_kind, ref)` to an agent and host session key `(harness, session_id)`. The wire request is snake_case and source is strict about unknown fields:

```json
{"method":"agent.register_session","params":{"agent_id":"agent-id","ref_kind":"opencode-session","ref":"session-reference","harness":"opencode","session_id":"session-id","project_root":"/absolute/project/root"}}
```

`agent_id`, `ref_kind`, `ref`, `harness`, and `session_id` are required. `project_root` is optional; omission preserves a known root when re-registering and otherwise leaves it unknown. The caller must be the resolved orchestrator or host provider, and identity authority must already be cut over. A repeated registration for the same agent/reference moves it to the new session key. Bindings are machine-local and are not journaled or shared to the identity log.

### Host feed into the volatile mirror

Only the module providing the daemon's sole `agent-host/v1` capability can send `projects.session_liveness`. Send a batch:

```json
{"method":"projects.session_liveness","params":{"seq":41,"snapshot":true,"sessions":[{"harness":"opencode","sessionId":"session-id","canonicalRoot":"/absolute/project/root","state":"active","lastActivityMs":1793100000000}]}}
```

The batch shape is `{seq, snapshot?, sessions?}`. `seq` is required and monotonic for deltas. Each session requires `sessionId`, `canonicalRoot`, and `state`; `harness` (default empty), `lastActivityMs` (default 0), `cause`, and `runner` are optional. A `gone` report removes that active mirror entry and remembers the ended key in a bounded gone set; ordinary activity states other than `active`/`idle` answer as unknown. The reply is `{accepted, snapshotRequested, tracked}` inside `result`.

On host startup, send a complete `snapshot: true` before relying on answers. The mirror starts stale at every Entorhinal module restart; it is in memory only and is neither journaled nor restored from disk. A snapshot replaces the whole mirror, including an empty snapshot. The first delta after startup may establish a sequence baseline but leaves the mirror stale; gaps and out-of-order/replayed deltas latch it stale and request a snapshot. Re-send a snapshot whenever the reply requests one and after host route/provider changes. The host, not each uc-studio consumer process, owns this feed.

### Query liveness by reference

```json
{"method":"agent.session_liveness","params":{"refs":[{"ref_kind":"opencode-session","ref":"session-reference"}]}}
```

`refs` must contain 1–256 objects with `ref_kind` and `ref`. Result has `refs` in request order, each with `state` and optional `cause`. The state is exactly one of `active`, `gone`, `unknown`. `active` means a non-stale mirror reported `active` or `idle`; `gone` means known ended or a binding without a host session; no binding, stale mirror, forgotten gone entry, or unrecognized feed state gives `unknown`. The host report can itself be old: this query does not poll it.

```ts
type SessionRef = { ref_kind: string; ref: string };
type LivenessAnswer = { state: "active" | "gone" | "unknown"; cause?: string };

async sessionLiveness(refs: SessionRef[]): Promise<LivenessAnswer[]> {
  if (refs.length < 1 || refs.length > 256) throw new RangeError("refs must contain 1..256 items");
  const result = await this.call<{ refs: LivenessAnswer[] }>("agent.session_liveness", { refs });
  return result.refs;
}

async routeToAgent(ref: SessionRef): Promise<"try-active-session" | "wake-or-defer"> {
  const [answer] = await this.sessionLiveness([ref]);
  return answer.state === "active" ? "try-active-session" : "wake-or-defer";
}
```

Treat `unknown` conservatively (busy/uncertain), not as a dead session or permission to claim the same work. The mailbox's receiver-local lease remains the authority for processing ownership. Even `active` is only a routing hint; delivery and retries must be resolved through uc-studio's messaging/room mechanisms.

## Three-phase migration and gates

### Phase 1 — Shadow Discovery

- Build the adapter and connect via the existing SubC TypeScript client from the daemon connection descriptor. Keep legacy registry and heartbeat behavior authoritative.
- For every project path, query `resolve`; periodically compare `enumerate` against the legacy registry. Record ID, `via`, `gone`, root and generation, and classify implicit paths separately.
- Instrument shadow liveness queries for registered references while legacy heartbeat routing remains authoritative. Ensure the actual host capability provider owns feed publication; the local adapter must not attempt a host-only mutation.
- **Gate:** all known legacy paths map deterministically; unexplained ID/root divergence is zero or explicitly dispositioned; implicit and gone outcomes are handled; query errors/refusals are visible; SubC reconnect succeeds without holding application locks.

### Phase 2 — Write Cutover

- Make Entorhinal IDs authoritative for newly-created and resolved project records. Migrate existing records by path through `resolve`; only call `register` for confirmed new projects, and use `attach_root` when adding a checkout to an existing project. Preserve legacy data as rollback/export data, not as a competing authority.
- Enable host session registration and snapshot/delta feed through the authorized provider. Cut routing reads over to `agent.session_liveness`; keep mailbox lease claim and discussion-room incarnation validation unchanged.
- On restart, require a fresh host snapshot before treating liveness as authoritative. If Entorhinal is unavailable or returns `unknown`, use the documented conservative fallback/wake behavior rather than stale files as a second live authority.
- **Gate:** every persisted project points to an Entorhinal ID; writes are idempotently retryable; tests prove one correct active/gone/unknown response; daemon/module restart returns `unknown` until snapshot and fresh state; receiver lease and stale-room-post fences still pass.

### Phase 3 — Deprecation / Excise

- Remove heartbeat file creation, freshness parsing, file watchers, project-registry writes and presence-based routing. Remove compatibility reads only after telemetry shows no consumer depends on them.
- Retain a one-time registry export/backup per uc-studio's normal migration policy; do not keep it writable as a fallback registry. Keep `ck-uc-discussions` room incarnation fences and receiver-local mailbox claims.
- **Gate:** repository search/build finds no production read/write path for the legacy registry or heartbeat directory; integration smoke test covers resolve → register session/feed → liveness query → routing decision; daemon restart/resnapshot and reconnect pass; mailbox lease and room incarnation regression tests pass.

## Configuration, setup, onboarding and OpenCode 1 specifics

### 1. Configuration & runtime prerequisites
- **Daemon supervision**: `ck-entorhinal 0.1.46` runs as a supervised reserved module under stock `ck-subc 0.20.124` at `~/.local/share/cortexkit/bin/ck-entorhinal` (configured in `~/.config/cortexkit/subc.jsonc`).
- **Connection file**: The SubC client must discover the daemon via `~/.local/share/cortexkit/run/subc-connection.json` (preserves `XDG_RUNTIME_DIR` set in `~/Library/LaunchAgents/cortexkit.subc.plist`).
- **Database isolation**: Entorhinal manages its own SQLite WAL store at `~/.local/share/cortexkit/entorhinal/store.db`. `uc-studio` never reads or writes this SQLite file directly; all reads/writes are strictly SubC Channel 0 RPCs.
- **Verification**: Run `ck health entorhinal` (must report `ok`).

### 2. Fleet project onboarding procedure
To onboard existing fleet projects from `~/.ucs/project-registry.json` without losing project continuity:
1. **Idempotent resolution**: For each project root in `project-registry.json`, call `resolve` with `{ "canonicalRoot": "<absolute_path>" }`.
2. **Minting canonical IDs**: If `via` is not `exact_root` (or returns implicit), call `register` with:
   ```json
   {
     "method": "register",
     "params": {
       "name": "<project-name>",
       "roots": ["<canonical-path>"],
       "requestKey": "onboard-<project-name>-v1"
     }
   }
   ```
   Entorhinal mints a persistent `pj-...` ID and commits it to its local SQLite journal.
3. **Legacy string ID preservation**: Existing callers referencing string IDs (e.g. `subconscious-770c36a2`, `uc-studio-8ce41322`, `cloudhome-5aa53d2c`) can attach these as labels or map them via `projects.attach_root`, ensuring that existing room participant identifiers resolve without regression.
4. **Subdirectory resolution**: Once a root is registered, any file or subdirectory lookup automatically resolves to the owning project via `via: "walk"` without redundant registrations.

### 3. OpenCode 1 vs OpenCode 2 specifics (strict OpenCode 1 fleet standard)
The UltraCreative fleet operates **strictly on OpenCode 1.x** (OpenCode 1.18.x / 1.19.x). OpenCode 2 is neither deployed nor supported.
- **Harness identifier**: Wire contracts and database schema constraints (`CHECK(residence_harness IN ('opencode', 'broca'))`) require the literal harness string `"opencode"`. Do NOT use `"opencode-v1"`, `"opencode-1"`, or `"opencode-2"`.
- **Session reference kind**: Always use `"ref_kind": "opencode-session"`.
- **Session IDs**: OpenCode 1 sessions use the `ses_<alphanumeric>` format (e.g. `ses_eed4f0970ffek4FG1AFXvSoNBv`). Entorhinal treats session IDs as opaque strings.
- **Host wake behavior in OpenCode 1**: In OpenCode 1, invoking `opencode serve` spawns the background HTTP server daemon but does **not** create an interactive session or publish endpoint presence automatically. When `uc-studio` auto-wakes an offline peer, it must:
  1. Spawn `opencode serve` (if not already running).
  2. Explicitly bootstrap a session on that instance to obtain a valid `session_id`.
  3. Call `agent.register_session` (`harness: "opencode"`, `session_id: "<ses_id>"`, `ref_kind: "opencode-session"`) so Entorhinal's liveness mirror marks it routable.
- **Database isolation**: OpenCode 1 maintains session history in `~/.local/share/opencode/opencode.db` and credentials in `~/.local/share/opencode/auth.json`. Entorhinal operates completely independent of `opencode.db`, preventing lock contention and schema coupling.

## Configuration, setup, onboarding and OpenCode 1 specifics

### 1. Setup and runtime prerequisites
- **Daemon supervision**: `ck-entorhinal 0.1.46` runs as a supervised reserved module under stock `ck-subc 0.20.124` at `~/.local/share/cortexkit/bin/ck-entorhinal` (configured in `~/.config/cortexkit/subc.jsonc`).
- **Connection file**: The SubC client must discover the daemon via `~/.local/share/cortexkit/run/subc-connection.json` (preserves `XDG_RUNTIME_DIR` set in `~/Library/LaunchAgents/cortexkit.subc.plist`).
- **Store path**: Entorhinal manages its own SQLite WAL store at `~/.local/share/cortexkit/entorhinal/store.db`. `uc-studio` never accesses or locks this file directly; all reads and writes flow through SubC Channel 0 ManagementSurface RPCs.
- **Verification**: `ck health entorhinal` confirms module health (`ok`).

### 2. Onboarding fleet projects (legacy registry seeding)
To transition from `~/.ucs/project-registry.json` to Entorhinal without losing project continuity:
1. **One-time registration scan**:
   - `uc-studio` iterates over known project paths in `~/.ucs/project-registry.json` (or scans workspace roots such as `/Volumes/Topper2TB/Git/*`).
   - For each repository, call `projects.register`:
     ```json
     {
       "method": "register",
       "params": {
         "name": "project-name",
         "roots": ["/absolute/path/to/project"],
         "requestKey": "onboard-project-name-v1"
       }
     }
     ```
   - Entorhinal assigns a durable `pj-...` ID and commits it to its local SQLite journal.
2. **Backward compatibility & legacy IDs**:
   - Mailbox envelopes and room memberships keyed to legacy string IDs (e.g. `subconscious-770c36a2`, `uc-studio-8ce41322`) should be mapped to the canonical `pj-...` ID in `uc-studio`'s local configuration or project metadata table (`set_domain`).
3. **Walk resolution for subprojects**:
   - Once a repository root is registered, any subpath within that directory (e.g. `/Volumes/Topper2TB/Git/subconscious/modules/uc-discussions`) queried via `resolve` returns the parent project ID with `via: "walk"`. Explicit subpath registration is only needed if a subdirectory belongs to an independent project workspace.

### 3. OpenCode 1 vs 2 specifics (strict OpenCode 1 fleet standard)
The UltraCreative fleet operates **strictly on OpenCode 1.x** (OpenCode 1.18.x / 1.19.x). OpenCode 2 is neither deployed nor supported.
- **Harness identifier**:
  - In all Entorhinal RPC payloads, the harness identifier is strictly `"opencode"` (e.g. `"harness": "opencode"`).
  - The underlying database schema enforces `CHECK(residence_harness IN (opencode, broca))`. There is no `"opencode-v2"` or version discriminator in Entorhinal 0.1.46. OpenCode 1 is the canonical fleet harness.
- **Session reference kind**: Always use `"ref_kind": "opencode-session"`.
- **Session IDs**: OpenCode 1 session IDs follow the `ses_<alphanumeric>` format (e.g. `ses_eed4f0970ffek4FG1AFXvSoNBv`). Entorhinal treats these as opaque strings.
- **Host wake behavior in OpenCode 1**:
  - In OpenCode 1, launching `opencode serve` starts the background HTTP server daemon, but **does not create an interactive session or publish endpoint presence automatically**.
  - `uc-studio`'s wake orchestrator must explicitly bootstrap an initial session on the spawned server, obtain the `ses_...` ID, and call `agent.register_session` before marking the session routable.
- **Database isolation**:
  - OpenCode 1 maintains session history in `~/.local/share/opencode/opencode.db` and credentials in `~/.local/share/opencode/auth.json`. Entorhinal operates completely independent of `opencode.db`, preventing lock contention and schema coupling.

## Contract source index

These checked-in sources define the wire details above; use them as the authority if code changes:

- `submodules/entorhinal/test-vectors/project-resolve-v1/README.md` and `resolve.json`, `resolve-project-id.json`, `enumerate.json`: exact ManagementSurface requests, result envelopes and representative replies (with project-resolution vectors including version 0.1.46 additions).
- `submodules/entorhinal/crates/entorhinal-module/src/main.rs`: `ResolveParams`, `ResolveProjectIdParams`, `EnumerateParams`, handlers for `resolve`, `resolve_project_id`, `enumerate`, `attach_root`, `LivenessBatch` / `LivenessSession`, and the request routing/read-write path.
- `submodules/entorhinal/crates/entorhinal-core/src/mutations.rs`: camelCase `RegisterRequest` fields.
- `submodules/entorhinal/crates/entorhinal-core/src/root_keys.rs`: `AttachRootRequest` fields.
- `submodules/entorhinal/crates/entorhinal-module/src/sessions.rs` and `submodules/entorhinal/crates/entorhinal-core/src/lib.rs`: strict `RegisterSessionRequest`, `SessionLivenessRequest`/`SessionRef`, binding rules and answer behavior.
- `submodules/entorhinal/docs/session-liveness-contract.md`: feed sequencing, snapshot/staleness, volatility, state mapping and response semantics.
- `submodules/entorhinal/docs/module-surface.md`: method names, query/mutation classification, route admission and caller roles.
- `docs/reference/entorhinal-architecture-and-fleet-integration.md`: local fleet boundary with `ck-uc-discussions` and uc-studio recommendations.
