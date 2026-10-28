#![forbid(unsafe_code)]

use std::sync::{Arc, Mutex};
use subc_protocol::Principal;

use crate::{
    protocol::host::{
        HostFeedLivenessRequest, HostFeedLivenessResponse, HostRegisterSessionRequest,
        HostRegisterSessionResponse,
    },
    service::ServiceError,
    Storage,
};

/// Trait abstracting outbound Channel 0 forwarding to `ck-entorhinal`.
pub trait EntorhinalForwarder: Send + Sync {
    fn forward_register_session(
        &self,
        request: &HostRegisterSessionRequest,
    ) -> Result<HostRegisterSessionResponse, ServiceError>;

    fn forward_feed_liveness(
        &self,
        request: &HostFeedLivenessRequest,
    ) -> Result<HostFeedLivenessResponse, ServiceError>;
}

/// Default forwarder acknowledging receipt and recording session metrics.
#[derive(Debug, Default)]
pub struct DefaultEntorhinalForwarder;

impl EntorhinalForwarder for DefaultEntorhinalForwarder {
    fn forward_register_session(
        &self,
        request: &HostRegisterSessionRequest,
    ) -> Result<HostRegisterSessionResponse, ServiceError> {
        Ok(HostRegisterSessionResponse {
            ok: true,
            registered: true,
            agent_id: request.agent_id.clone(),
            session_id: request.session_id.clone(),
        })
    }

    fn forward_feed_liveness(
        &self,
        request: &HostFeedLivenessRequest,
    ) -> Result<HostFeedLivenessResponse, ServiceError> {
        let count = request.sessions.as_ref().map(|s| s.len()).unwrap_or(0);
        Ok(HostFeedLivenessResponse {
            accepted: true,
            snapshot_requested: false,
            tracked: count,
        })
    }
}

/// In-memory state tracking feed sequence and snapshot recovery.
#[derive(Debug, Default)]
struct FeedTracker {
    last_seq: Option<i64>,
    snapshot_requested: bool,
    active_sessions: usize,
}

pub struct HostService {
    _storage: Storage,
    forwarder: Arc<dyn EntorhinalForwarder>,
    tracker: Mutex<FeedTracker>,
}

impl HostService {
    pub fn new(storage: Storage, forwarder: Arc<dyn EntorhinalForwarder>) -> Self {
        Self {
            _storage: storage,
            forwarder,
            tracker: Mutex::new(FeedTracker {
                last_seq: None,
                snapshot_requested: true, // starts requiring a snapshot on startup
                active_sessions: 0,
            }),
        }
    }

    pub fn with_default_forwarder(storage: Storage) -> Self {
        Self::new(storage, Arc::new(DefaultEntorhinalForwarder))
    }

    /// Authorize that caller is permitted to issue host feed/registration mutations.
    /// Rejects unauthorized or foreign callers.
    pub fn check_caller_authorized(&self, principal: Option<&Principal>) -> Result<(), ServiceError> {
        let Some(principal) = principal else {
            return Err(ServiceError::CallerForbidden(
                "unauthenticated caller: no route principal".to_owned(),
            ));
        };
        match principal {
            Principal::Direct => Ok(()),
            Principal::Reserved { module_id }
                if module_id == "uc-discussions" || module_id == "prefrontal-core" =>
            {
                Ok(())
            }
            other => Err(ServiceError::CallerForbidden(format!(
                "unauthorized principal '{:?}': host mutations require direct caller or admitted host",
                other
            ))),
        }
    }

    /// Register a host session binding with Entorhinal.
    pub fn register_session(
        &self,
        caller: Option<&Principal>,
        request: HostRegisterSessionRequest,
    ) -> Result<HostRegisterSessionResponse, ServiceError> {
        self.check_caller_authorized(caller)?;

        if request.agent_id.trim().is_empty() {
            return Err(ServiceError::InvalidRequest("agent_id must not be empty".to_owned()));
        }
        if request.ref_kind.trim().is_empty() {
            return Err(ServiceError::InvalidRequest("ref_kind must not be empty".to_owned()));
        }
        if request.session_ref.trim().is_empty() {
            return Err(ServiceError::InvalidRequest("ref must not be empty".to_owned()));
        }
        if request.session_id.trim().is_empty() {
            return Err(ServiceError::InvalidRequest("session_id must not be empty".to_owned()));
        }
        // Strict OpenCode 1 fleet constraint:
        if request.harness != "opencode" {
            return Err(ServiceError::InvalidRequest(format!(
                "unsupported harness '{}': fleet standard strictly requires 'opencode'",
                request.harness
            )));
        }

        self.forwarder.forward_register_session(&request)
    }

    /// Feed volatile session liveness batch to Entorhinal.
    pub fn feed_liveness(
        &self,
        caller: Option<&Principal>,
        request: HostFeedLivenessRequest,
    ) -> Result<HostFeedLivenessResponse, ServiceError> {
        self.check_caller_authorized(caller)?;

        let mut tracker = self.tracker.lock().map_err(|_| {
            ServiceError::InvalidRequest("host service tracker lock poisoned".to_owned())
        })?;

        // Monotonic sequence and snapshot recovery validation
        let is_snapshot = request.snapshot.unwrap_or(false);
        if !is_snapshot {
            if let Some(last) = tracker.last_seq {
                if request.seq != last + 1 {
                    // Sequence gap detected: latch snapshot_requested
                    tracker.snapshot_requested = true;
                    return Ok(HostFeedLivenessResponse {
                        accepted: false,
                        snapshot_requested: true,
                        tracked: tracker.active_sessions,
                    });
                }
            } else if tracker.snapshot_requested {
                // First feed after startup must be a snapshot
                return Ok(HostFeedLivenessResponse {
                    accepted: false,
                    snapshot_requested: true,
                    tracked: 0,
                });
            }
        }

        // Validate sessions
        if let Some(ref sessions) = request.sessions {
            for s in sessions {
                if s.harness != "opencode" {
                    return Err(ServiceError::InvalidRequest(format!(
                        "unsupported session harness '{}': fleet standard strictly requires 'opencode'",
                        s.harness
                    )));
                }
            }
        }

        let resp = self.forwarder.forward_feed_liveness(&request)?;

        if resp.accepted {
            tracker.last_seq = Some(request.seq);
            tracker.snapshot_requested = resp.snapshot_requested;
            tracker.active_sessions = resp.tracked;
        }

        Ok(resp)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::protocol::host::LivenessSession;

    use std::sync::atomic::{AtomicUsize, Ordering};

    struct MockForwarder {
        register_calls: AtomicUsize,
        feed_calls: AtomicUsize,
    }

    impl MockForwarder {
        fn new() -> Self {
            Self {
                register_calls: AtomicUsize::new(0),
                feed_calls: AtomicUsize::new(0),
            }
        }
    }

    impl EntorhinalForwarder for MockForwarder {
        fn forward_register_session(
            &self,
            request: &HostRegisterSessionRequest,
        ) -> Result<HostRegisterSessionResponse, ServiceError> {
            self.register_calls.fetch_add(1, Ordering::SeqCst);
            Ok(HostRegisterSessionResponse {
                ok: true,
                registered: true,
                agent_id: request.agent_id.clone(),
                session_id: request.session_id.clone(),
            })
        }

        fn forward_feed_liveness(
            &self,
            request: &HostFeedLivenessRequest,
        ) -> Result<HostFeedLivenessResponse, ServiceError> {
            self.feed_calls.fetch_add(1, Ordering::SeqCst);
            let count = request.sessions.as_ref().map(|s| s.len()).unwrap_or(0);
            Ok(HostFeedLivenessResponse {
                accepted: true,
                snapshot_requested: false,
                tracked: count,
            })
        }
    }

    fn test_service() -> (HostService, Arc<MockForwarder>) {
        let storage = Storage::open_in_memory().unwrap();
        let forwarder = Arc::new(MockForwarder::new());
        let service = HostService::new(storage, forwarder.clone());
        (service, forwarder)
    }

    #[test]
    fn unauthorized_callers_are_strictly_rejected() {
        let (service, _) = test_service();

        // 1. Unauthenticated (None)
        let err_unauth = service.check_caller_authorized(None);
        assert!(matches!(err_unauth, Err(ServiceError::CallerForbidden(_))));

        // 2. Foreign unverified or unauthorized module principal
        let foreign_principal = Principal::Reserved {
            module_id: "untrusted-module".to_owned(),
        };
        let err_foreign = service.check_caller_authorized(Some(&foreign_principal));
        assert!(matches!(err_foreign, Err(ServiceError::CallerForbidden(_))));

        let unverified = Principal::Unverified;
        let err_unverified = service.check_caller_authorized(Some(&unverified));
        assert!(matches!(err_unverified, Err(ServiceError::CallerForbidden(_))));

        // 3. Direct caller authorized
        let ok_direct = service.check_caller_authorized(Some(&Principal::Direct));
        assert!(ok_direct.is_ok());
    }

    #[test]
    fn register_session_enforces_opencode_harness_and_required_fields() {
        let (service, forwarder) = test_service();
        let caller = Principal::Direct;

        // Unsupported harness (e.g. "opencode-2" or "pi")
        let bad_harness = HostRegisterSessionRequest {
            agent_id: "agent-1".to_owned(),
            ref_kind: "opencode-session".to_owned(),
            session_ref: "subconscious:agent".to_owned(),
            harness: "opencode-v2".to_owned(),
            session_id: "ses_123".to_owned(),
            project_root: None,
        };
        let err_harness = service.register_session(Some(&caller), bad_harness);
        assert!(matches!(err_harness, Err(ServiceError::InvalidRequest(_))));

        // Valid OpenCode 1 registration
        let valid_req = HostRegisterSessionRequest {
            agent_id: "agent-1".to_owned(),
            ref_kind: "opencode-session".to_owned(),
            session_ref: "subconscious:agent".to_owned(),
            harness: "opencode".to_owned(),
            session_id: "ses_123".to_owned(),
            project_root: Some("/Volumes/Topper2TB/Git/subconscious".to_owned()),
        };
        let res = service.register_session(Some(&caller), valid_req).expect("valid registration");
        assert_eq!(res.registered, true);
        assert_eq!(res.session_id, "ses_123");
        assert_eq!(forwarder.register_calls.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn liveness_feed_requires_initial_snapshot_and_detects_sequence_gaps() {
        let (service, forwarder) = test_service();
        let caller = Principal::Direct;

        // 1. First feed after startup without snapshot=true is refused with snapshotRequested=true
        let delta_feed = HostFeedLivenessRequest {
            seq: 1,
            snapshot: Some(false),
            sessions: Some(vec![LivenessSession {
                harness: "opencode".to_owned(),
                session_id: "ses_1".to_owned(),
                canonical_root: "/repo".to_owned(),
                state: "active".to_owned(),
                last_activity_ms: Some(100),
            }]),
        };
        let res_first = service.feed_liveness(Some(&caller), delta_feed).unwrap();
        assert_eq!(res_first.accepted, false);
        assert_eq!(res_first.snapshot_requested, true);

        // 2. Initial snapshot succeeds
        let snapshot_feed = HostFeedLivenessRequest {
            seq: 1,
            snapshot: Some(true),
            sessions: Some(vec![LivenessSession {
                harness: "opencode".to_owned(),
                session_id: "ses_1".to_owned(),
                canonical_root: "/repo".to_owned(),
                state: "active".to_owned(),
                last_activity_ms: Some(100),
            }]),
        };
        let res_snap = service.feed_liveness(Some(&caller), snapshot_feed).unwrap();
        assert_eq!(res_snap.accepted, true);
        assert_eq!(res_snap.snapshot_requested, false);
        assert_eq!(forwarder.feed_calls.load(Ordering::SeqCst), 1);

        // 3. Consecutive delta (seq=2) succeeds
        let delta_next = HostFeedLivenessRequest {
            seq: 2,
            snapshot: Some(false),
            sessions: Some(vec![]),
        };
        let res_delta = service.feed_liveness(Some(&caller), delta_next).unwrap();
        assert_eq!(res_delta.accepted, true);

        // 4. Sequence gap (seq=5 instead of 3) triggers snapshot_requested
        let gap_feed = HostFeedLivenessRequest {
            seq: 5,
            snapshot: Some(false),
            sessions: Some(vec![]),
        };
        let res_gap = service.feed_liveness(Some(&caller), gap_feed).unwrap();
        assert_eq!(res_gap.accepted, false);
        assert_eq!(res_gap.snapshot_requested, true);
    }
}
