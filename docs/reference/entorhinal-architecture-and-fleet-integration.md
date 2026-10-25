# Entorhinal Architecture & Fleet Integration Reference

This reference documents the architecture, capabilities, data model, and integration boundaries of
`cortexkit/entorhinal` (the authoritative Project and Agent Registry module). It establishes how
Entorhinal functions internally, how it interacts with the rest of the CortexKit module suite, how it
boundaries with UltraCreative Studio's custom deliberation engine (`ck-uc-discussions`), and how it
informs the `uc-studio` roadmap for central project management and agent communications.

---

## 1. What Entorhinal Is & What It Owns

Entorhinal (`ck-entorhinal`, supervised under `ck-subc`) is the **authoritative identity and liveness
registry** for the CortexKit fleet. It provides four formal capabilities over SubC Channel 0:
- `project-identity/v1` (Durable project IDs `pj-...`, canonical root mapping, workspace assignments)
- `agent-identity/v1` (Agent definitions, hierarchies: assistant -> head -> hire, roles, tags, avatars)
- `permissions/v1` (Operator consent cards, standing grants, audit trail, hire caps)
- `tool-provider/v1` (Agent-callable tools for creating workspaces, registering projects, and hiring)

### Core Invariants & Boundaries

1. **Reads Never Block**:
   Read operations run on dedicated, read-only SQLite WAL connections that see committed snapshots.
   Queries never wait for the single-writer lock, remote machines, or operator presence prompts. Health
   checks (`ck health entorhinal`) compute purely from in-memory counters.
2. **Paths and Authority Never Travel**:
   While project and agent identity are shared across the fleet via the identity log (`engram`),
   **filesystem root paths, operator approvals, standing grants, and permission caps are machine-local**.
   A machine received entry from another host never authorizes a local write or path mapping.
3. **Identity Is Not Residence**:
   An agent's identity (name, role, hire hierarchy) is global; which physical machine currently holds
   the agent is a separate claim epoch. Agents move between machines via the structured 6-state move
   protocol (`published` -> `staged` -> `claimed` -> `activated` -> `released`).
4. **Session Liveness Is an In-Memory Mirror**:
   Entorhinal tracks session bindings (`(harness, session_id)` <-> agent reference). The liveness
   state (`active`, `gone`, `unknown`) is pushed by the host/orchestrator into an in-memory mirror and
   is deliberately excluded from the disk journal to prevent stale state from surviving daemon restarts.
5. **No Forked Code**:
   We build and run Entorhinal directly from upstream Git HEAD without local fork patches. It is tracked
   as a submodule in `submodules/entorhinal`.

---

## 2. Data Model & Architecture

### Hierarchy of Identity

```
Operator
   └── Workspaces (ws-...)
         ├── Accepted Workspace Documents (charter + policy.md)
         ├── Projects (pj-...)
         │     ├── Machine Roots (canonical local paths, e.g. /Volumes/Topper2TB/Git/subconscious)
         │     ├── Remotes (git remote URLs, e.g. github.com/rustybret/subconscious)
         │     ├── Project Domain (@scope/package canonical catalog name)
         │     └── Project Head Agent (role = head)
         │           └── Hires (role = hire, bounded by hire-cap)
         └── Assistant Agents (role = assistant)
```

### Module Operations Matrix

| Namespace | Key Verbs | Purpose |
|---|---|---|
| `projects.*` | `resolve`, `resolve_project_id`, `resolve_remote`, `enumerate`, `register`, `attach_root`, `set_domain` | Path-to-project translation, directory mapping, repo ownership |
| `workspaces.*` | `list`, `assign`, `set_document`, `document` | Multi-project grouping and operator policy documentation |
| `agent.*` | `create`, `rename`, `retire`, `resolve`, `fleet_identity`, `register_session`, `session_liveness` | Agent lifecycle, session binding, and real-time activity mirror |
| `permissions.*` | `grants`, `grant`, `revoke_grant`, `audit`, `hire_cap` | Operator permission controls, audit ledger, and delegation bounds |

---

## 3. How Entorhinal Relates to the CortexKit Module Suite

Entorhinal does not operate in isolation; it provides the foundational identity layer upon which the other
specialized modules rely:

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                             ck-subc Supervisor                              │
│                                                                             │
│  ┌────────────────────────┐                  ┌───────────────────────────┐  │
│  │     ck-entorhinal      │◄── Identity ────┤       ck-claustrum        │  │
│  │ (Project/Agent Registry)│    Reference     │    (Credential Vault)     │  │
│  └───────────┬────────────┘                  └───────────────────────────┘  │
│              │                                                              │
│              │ Project ID &                                                 │
│              │ Workspace Policy                                             │
│              ▼                                                              │
│  ┌────────────────────────┐                  ┌───────────────────────────┐  │
│  │         ck-mc          │                  │        ck-fusiform        │  │
│  │    (Magic Context)     │                  │  (Model & Pricing Catalog)│  │
│  └───────────┬────────────┘                  └───────────────────────────┘  │
│              │                                                              │
│              │ Shared SQLite / Loopback Transport                           │
│              ▼                                                              │
│  ┌────────────────────────┐                  ┌───────────────────────────┐  │
│  │          aft           │                  │       ck-astrocyte        │  │
│  │  (Agent File Tools)    │                  │  (Token & Cost Accounting)│  │
│  └────────────────────────┘                  └───────────────────────────┘  │
└─────────────────────────────────────────────────────────────────────────────┘
```

1. **`ck-subc` (Core Daemon)**:
   - Supervises `ck-entorhinal` and routes Channel 0 ManagementSurface calls to it.
   - Entorhinal registers `project-identity/v1` and `agent-identity/v1`. Dependent modules declaring these
     capabilities as `required` remain unroutable until Entorhinal successfully completes its HELLO handshake.
   - Provides presence prompts (Touch ID / Windows Hello) for operator-confirmed mutations.
2. **`ck-claustrum` (Credential Vault)**:
   - When consumers enroll (`auth.enroll_propose`) or query credentials (`credential.list_scoped`),
     Claustrum verifies caller identity against SubC route principals.
   - Scoped grants bind to principals (`enrolled:<name>` or `reserved:<module>`), ensuring credentials
     remain partitioned by project and agent boundary.
3. **`ck-mc` (Magic Context)**:
   - Uses Entorhinal's `project.domain` and `workspace.document` (Memory Context v1) to calibrate context
     generation and dream windows.
   - Binds session records to durable project IDs (`pj-...`) rather than raw paths, ensuring continuity
     even if a repository folder is renamed or moved.
4. **`aft` (Agent File Tools)**:
   - AFT resolves project roots via `projects.resolve` to establish LSP workspace boundaries, file
     indexes, and AST analysis roots.
5. **`ck-fusiform` & `ck-astrocyte`**:
   - `fusiform` normalizes model capabilities and pricing rates.
   - `astrocyte` tracks spend against project IDs resolved by Entorhinal, attributing LLM consumption
     accurately to specific projects and agents.

---

## 4. Boundary with Custom Modules: `ck-uc-discussions` & `agent-inbox/v1`

UltraCreative Studio authored `ck-uc-discussions` to solve multi-agent deliberation. It is critical to
understand the strict seam between Entorhinal and `uc-discussions`:

| Feature / Responsibility | Owned by `entorhinal` | Owned by `ck-uc-discussions` |
|---|---|---|
| **Identity Authority** | Canonical project IDs (`pj-...`), agent names, roles, hire caps | None. Consumes Entorhinal's IDs as strings. |
| **Session Tracking** | In-memory `session_liveness` feed, harness-to-session keys | Monotonic incarnation fences per room (`incarnation >= 1`). |
| **Room State** | None. Completely unaware of rooms or debate topics. | Owns rooms, membership rosters, ordered turns, objections, votes. |
| **Turn Immutability** | None. | Strict sequential post allocation under SQLite transaction locks. |
| **Message Routing** | None. Never handles envelopes or turn dispatches. | Serves `rooms.*`, `peer.*`, and `agent-inbox/v1` (`inbox.post/read`). |
| **Storage Engine** | Synchronous WAL SQLite (`~/.local/share/cortexkit/entorhinal/`) | Synchronous WAL SQLite (`~/.local/share/cortexkit/uc-discussions/`) |

### Key Architectural Separation

- **Do Not Fold Registry into Discussions**: Entorhinal answers *who* an agent or project is. `uc-discussions`
  answers *what was decided* in a multi-party deliberation room.
- **Incarnation Fencing**: While Entorhinal tracks whether a session is currently `active` or `gone`,
  `uc-discussions` uses monotonic incarnation numbers on room members to reject stale posts from sessions
  that died and reconnected during an active vote.
- **Neutral Drop Box (`agent-inbox/v1`)**: `uc-discussions` implements `cortexkit-role-agent-inbox`, allowing
  agents identified by Entorhinal to drop neutral, idempotent JSON payloads to peer mailboxes without
  touching physical project directory files.

---

## 5. Blueprint for `uc-studio`: The Central Manager Agent & Agent Comms

`uc-studio` is currently executing review gates on its `agent-comms-redesign` and roadmap for a Central
Project Manager Agent. Entorhinal's architecture provides the exact building blocks required for this
system:

### 1. The Manager Agent as a "Project Head"

In Entorhinal's data model:
- Each project can designate an agent with `role: "head"` (`agents_create_head`).
- A `head` agent has unique tool authority to hire worker agents (`agents_hire`), retire them
  (`agents_retire_hire`), and inspect the active hire roster (`agents_list_hires`).
- Operator hire caps (`permissions.hire_cap`) prevent autonomous runaway delegation.

`uc-studio`'s Central Manager Agent should be registered as the `head` agent of the workspace. Worker
subagents spawned for tasks (researchers, testers, implementers) should be registered as `hire` agents
under that head.

### 2. Auto-Wake & Dynamic Presence Orchestration

Entorhinal's `session-liveness-contract.md` establishes the canonical orchestration pattern:

1. **Host Feed (`projects.session_liveness`)**:
   `uc-studio` (acting as the host provider of `agent-host/v1`) pushes session state updates to Entorhinal.
2. **Liveness Queries (`agent.session_liveness`)**:
   When an agent wants to communicate with a colleague, it queries Entorhinal for the colleague's session
   liveness (`active`, `gone`, or `unknown`).
3. **Targeted Wake via `project_room`**:
   - If the target colleague is `active`, dispatch directly to the active session.
   - If `gone` or `unknown`, `uc-studio` triggers the host wake primitive (`opencode serve`), registers
     the new session binding with Entorhinal (`agent.register_session`), and convenes the room.

### 3. Deliberation Rooms vs Mailbox Doorbell

The recommended inter-agent communication protocol follows our established 4-point standard:
1. **Durable Turns in `uc-discussions`**: All multi-agent deliberation, proposals, reviews, and decision
   synthesis occur as monotonic posts inside `ck-uc-discussions` rooms (`seq 1, 2, 3...`).
2. **Doorbell Pings via `project_message` / `agent-inbox`**: Ephemeral notifications ("Room X has a new turn
   awaiting your review") are dropped via `project_message` or `inbox.post`, acting strictly as wake-up
   signals that carry the room ID rather than giant payload dumps.
3. **Bounded Micro-Turns**: Agent replies in rooms should stay bounded (under 250 words) with explicit
   Delta, Agreement/Dissent, and Next Action sections to prevent token sprawl.
4. **Fast-Path Closure**: Discussions close in 3–5 turns, producing an immutable closure summary and dissent
   record in SQLite that any fleet agent can audit.

### 4. Cross-Machine Mobility (Multi-Workstation Fleet)

For multi-machine setups (this Mac + Cloudhome worker nodes + other workstations):
- Entorhinal's `agent-move-contract.md` allows idle agents to be migrated cleanly between machines along
  with their session history and pins, using engram's identity log.
- Machines do not share raw SQLite stores or file paths; each runs its own local Entorhinal and SubC
  supervisor, synchronizing agent identities globally while keeping runtime process execution local.

---

## 6. Summary of Actionable Recommendations for Fleet Projects

1. **For `uc-studio`**:
   - Align the Central Manager Agent with Entorhinal's `head` role and hire hierarchy.
   - Consume `agent.session_liveness` from Entorhinal rather than maintaining ad-hoc process polling loops.
   - Use `ck-uc-discussions` for multi-agent council rounds, and restrict mailbox messages to wake doorbells.
2. **For `subconscious`**:
   - Maintain `submodules/entorhinal` tracking upstream `origin/main` without local fork patches.
   - Keep `ck-uc-discussions` strictly scoped to durable room deliberation and neutral inbox drops.
3. **For `cloudhome`**:
   - Utilize Entorhinal's durable project IDs (`pj-...`) in Jenkins build jobs and artifact catalog tagging.
   - Preserve machine-local path isolation when deploying containerized agent workers.
