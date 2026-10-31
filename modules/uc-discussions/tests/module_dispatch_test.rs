#![forbid(unsafe_code)]

use serde::de::DeserializeOwned;
use serde_json::{json, Value};
use subc_client_rs::HandlerOutcome;
use subc_protocol::manifest::{Concurrency, ProviderRole};
use subc_protocol::Principal;
use uc_discussions::{
    manifest,
    protocol::{
        council::{EvaluateCouncilResponse, ReconcileCouncilResponse, StageCouncilResponse},
        peer::{EnqueueMessageResponse, PollInboxResponse},
        rooms::{CreateRoomResponse, PostRoomResponse},
    },
    DiscussionsHandler,
};

fn dispatch(handler: &DiscussionsHandler, request: Value) -> HandlerOutcome {
    handler.dispatch(
        &serde_json::to_vec(&request).expect("test request should serialize to JSON bytes"),
    )
}

fn response<T: DeserializeOwned>(outcome: HandlerOutcome) -> T {
    match outcome {
        HandlerOutcome::Response(bytes) => {
            serde_json::from_slice(&bytes).expect("handler response should match the expected type")
        }
        other => panic!("expected response outcome, got {other:?}"),
    }
}

#[test]
fn manifest_declares_all_management_surface_operations() {
    let manifest = manifest();
    assert_eq!(manifest.module_id, "uc-discussions");
    let [ProviderRole::ManagementSurface {
        operations,
        concurrency,
        ..
    }] = manifest.provides.as_slice()
    else {
        panic!("manifest should declare exactly one management surface");
    };

    assert_eq!(*concurrency, Concurrency::ModuleManaged);
    assert_eq!(
        operations
            .iter()
            .map(|operation| operation.name.as_str())
            .collect::<Vec<_>>(),
        vec![
            "peer.enqueue_message",
            "peer.poll_inbox",
            "peer.ack_message",
            "peer.acquire_lease",
            "peer.renew_lease",
            "peer.release_lease",
            "rooms.create",
            "rooms.join",
            "rooms.bind_member",
            "rooms.post",
            "rooms.object",
            "rooms.revise",
            "rooms.poll",
            "rooms.vote",
            "rooms.grant_stage",
            "rooms.close",
            "rooms.get",
            "council.stage",
            "council.evaluate",
            "council.reconcile",
            "council.get",
            "inbox.post",
            "inbox.withdraw",
            "inbox.list",
            "inbox.read",
            "host.register_session",
            "host.feed_liveness",
        ]
    );
    assert_eq!(
        manifest.capabilities.unwrap().provides,
        vec!["agent-host/v1".to_owned()]
    );
}

#[test]
fn enqueue_message_dispatch_persists_message() {
    let handler = DiscussionsHandler::default();
    let enqueued: EnqueueMessageResponse = response(dispatch(
        &handler,
        json!({
            "op": "peer.enqueue_message",
            "params": {
                "from_name": "sender",
                "to_name": "recipient",
                "session_id": "recipient-session",
                "body": "hello from dispatch"
            }
        }),
    ));
    assert!(enqueued.ok);
    assert_eq!(enqueued.state, "pending");

    let inbox: PollInboxResponse = response(dispatch(
        &handler,
        json!({
            "op": "peer.poll_inbox",
            "session_id": "recipient-session",
            "limit": 10,
            "after_id": null
        }),
    ));
    assert_eq!(inbox.messages.len(), 1);
    assert_eq!(inbox.messages[0].message_id, enqueued.message_id);
    assert_eq!(inbox.messages[0].body, "hello from dispatch");
}

#[test]
fn room_dispatch_allocates_monotonic_post_sequences() {
    let handler = DiscussionsHandler::default();
    let room: CreateRoomResponse = response(dispatch(
        &handler,
        json!({
            "op": "rooms.create",
            "params": {
                "topic": "Release",
                "goal": "Decide whether to ship",
                "stage": "discussion",
                "creator": "alice"
            }
        }),
    ));

    let first: PostRoomResponse = response(dispatch(
        &handler,
        json!({
            "op": "rooms.post",
            "room_id": room.room_id,
            "author": "alice",
            "post_type": "proposal",
            "content": "Ship after verification",
            "reply_to_post_id": null
        }),
    ));
    let second: PostRoomResponse = response(dispatch(
        &handler,
        json!({
            "op": "rooms.post",
            "params": {
                "room_id": room.room_id,
                "author": "bob",
                "post_type": "evidence",
                "content": "All checks passed",
                "reply_to_post_id": first.post.post_id
            }
        }),
    ));

    assert_eq!(first.seq, 1);
    assert_eq!(second.seq, 2);
    assert!(second.seq > first.seq);
}

#[test]
fn council_dispatch_requires_all_terminal_members_before_reconcile() {
    let handler = DiscussionsHandler::default();
    let staged: StageCouncilResponse = response(dispatch(
        &handler,
        json!({
            "op": "council.stage",
            "params": {
                "council_id": "release-council",
                "name": "Release council",
                "question": "Should we ship?",
                "intent": "decision",
                "mode": "plan",
                "members": ["alpha", "beta"],
                "prompt": "Evaluate release readiness",
                "context_files": null,
                "guidance": null,
                "deadline_ms": null
            }
        }),
    ));
    assert_eq!(staged.status, "staged");

    let alpha: EvaluateCouncilResponse = response(dispatch(
        &handler,
        json!({
            "op": "council.evaluate",
            "council_id": "release-council",
            "member_name": "alpha",
            "status": "completed",
            "response_block": "approve",
            "error": null,
            "token_cost_nanodollars": 10
        }),
    ));
    assert!(!alpha.all_members_terminal);

    let early_reconcile = dispatch(
        &handler,
        json!({
            "op": "council.reconcile",
            "params": {
                "council_id": "release-council",
                "declared_members": ["alpha", "beta"],
                "synthesis": "ship",
                "agreement_level": "strong"
            }
        }),
    );
    match early_reconcile {
        HandlerOutcome::Error { code, message } => {
            assert_eq!(code, "invalid_request");
            assert!(message.contains("beta"));
            assert!(message.contains("non-terminal"));
        }
        other => panic!("expected non-terminal reconciliation error, got {other:?}"),
    }

    let beta: EvaluateCouncilResponse = response(dispatch(
        &handler,
        json!({
            "op": "council.evaluate",
            "params": {
                "council_id": "release-council",
                "member_name": "beta",
                "status": "failed",
                "response_block": null,
                "error": "model unavailable",
                "token_cost_nanodollars": null
            }
        }),
    ));
    assert!(beta.all_members_terminal);

    let reconciled: ReconcileCouncilResponse = response(dispatch(
        &handler,
        json!({
            "op": "council.reconcile",
            "council_id": "release-council",
            "declared_members": ["alpha", "beta"],
            "synthesis": "ship with alpha's approval and beta's failure recorded",
            "agreement_level": "partial"
        }),
    ));
    assert!(reconciled.ok);
    assert_eq!(reconciled.status, "completed");
}

#[test]
fn unknown_operation_returns_clear_error() {
    let handler = DiscussionsHandler::default();
    match dispatch(&handler, json!({ "op": "rooms.no_such_operation" })) {
        HandlerOutcome::Error { code, message } => {
            assert_eq!(code, "unknown_operation");
            assert_eq!(message, "unknown operation 'rooms.no_such_operation'");
        }
        other => panic!("expected unknown-operation error, got {other:?}"),
    }
}

#[test]
fn inbox_dispatch_posts_and_reads_item() {
    let handler = DiscussionsHandler::default();
    let post_req = serde_json::json!({
        "op": "inbox.post",
        "params": {
            "scope": {
                "scope_ref": "test-scope",
                "epoch": 1
            },
            "notice": {
                "idempotency_key": "dispatch-test-1",
                "content": {
                    "text": "Hello via SubC Dispatch"
                },
                "attention": "silent"
            }
        }
    });

    let outcome = dispatch(&handler, post_req);
    let post_reply: serde_json::Value = response(outcome);
    assert_eq!(post_reply["state"], "accepted");
    let item_id = post_reply["item_id"].as_str().expect("item_id");

    let read_req = serde_json::json!({
        "op": "inbox.read",
        "params": {
            "item_id": item_id
        }
    });
    let read_reply: serde_json::Value = response(dispatch(&handler, read_req));
    let item = read_reply["item"].as_object().expect("item object");
    assert_eq!(item["item_id"], item_id);
    assert!(item["body"].as_str().unwrap().contains("Hello via SubC Dispatch"));
}

#[test]
fn host_relay_dispatch_checks_authorization_and_forwards() {
    
    let handler = DiscussionsHandler::default();
    let register_req = json!({
        "op": "host.register_session",
        "params": {
            "agent_id": "athena-junior",
            "ref_kind": "opencode-session",
            "ref": "subconscious:athena",
            "harness": "opencode",
            "session_id": "ses_12345",
            "project_root": "/workspace/subconscious"
        }
    });
    let req_bytes = serde_json::to_vec(&register_req).unwrap();

    // 1. Unauthenticated caller is rejected with caller_forbidden
    let unauth_outcome = handler.dispatch_with_caller(&req_bytes, None);
    match unauth_outcome {
        HandlerOutcome::Error { code, message } => {
            assert_eq!(code, "caller_forbidden");
            assert!(message.contains("unauthenticated caller"));
        }
        other => panic!("expected caller_forbidden error, got {other:?}"),
    }

    // 2. Direct caller is authorized
    let direct_caller = Principal::Direct;
    let auth_outcome = handler.dispatch_with_caller(&req_bytes, Some(&direct_caller));
    let reg_reply: Value = response(auth_outcome);
    assert_eq!(reg_reply["ok"], true);
    assert_eq!(reg_reply["registered"], true);
    assert_eq!(reg_reply["session_id"], "ses_12345");

    // 3. Liveness feed snapshot with direct caller
    let feed_req = json!({
        "op": "host.feed_liveness",
        "params": {
            "seq": 1,
            "snapshot": true,
            "sessions": [
                {
                    "harness": "opencode",
                    "sessionId": "ses_12345",
                    "canonicalRoot": "/workspace/subconscious",
                    "state": "active",
                    "lastActivityMs": 1793140000000_i64
                }
            ]
        }
    });
    let feed_bytes = serde_json::to_vec(&feed_req).unwrap();
    let feed_outcome = handler.dispatch_with_caller(&feed_bytes, Some(&direct_caller));
    let feed_reply: Value = response(feed_outcome);
    assert_eq!(feed_reply["accepted"], true);
    assert_eq!(feed_reply["snapshotRequested"], false);
    assert_eq!(feed_reply["tracked"], 1);
}

