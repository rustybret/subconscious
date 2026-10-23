#![forbid(unsafe_code)]

use std::{fs, process::Command};

use serde_json::json;
use subc_client_rs::{HandlerOutcome, ModuleHandler};
use subc_protocol::{session::HealthStatus, ModuleHelloAckBody};
use uc_discussions::DiscussionsHandler;

fn assert_probe(flag: &str, expected: &str) {
    // Given an isolated working directory and no daemon configuration.
    let root = std::env::temp_dir().join(format!("uc-discussions-cli-{}", uuid::Uuid::new_v4()));
    fs::create_dir(&root).expect("create CLI sandbox");

    // When the real binary is invoked without --subc.
    let output = Command::new(env!("CARGO_BIN_EXE_ck-uc-discussions"))
        .arg(flag)
        .current_dir(&root)
        .env("HOME", &root)
        .env("XDG_DATA_HOME", &root)
        .env("XDG_CONFIG_HOME", &root)
        .output()
        .expect("execute CLI probe");
    let entries = fs::read_dir(&root).expect("inspect CLI sandbox").count();
    fs::remove_dir_all(&root).expect("remove CLI sandbox");

    // Then it succeeds with the requested output and no storage artifacts.
    assert!(
        output.status.success(),
        "{flag} must exit 0 without a daemon: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(String::from_utf8_lossy(&output.stdout).contains(expected));
    assert_eq!(
        entries, 0,
        "CLI probes must not create a database or directories"
    );
}

#[test]
fn cli_version_without_daemon() {
    assert_probe(
        "--version",
        concat!("ck-uc-discussions ", env!("CARGO_PKG_VERSION")),
    );
}

#[test]
fn cli_help_without_daemon() {
    assert_probe("--help", "--subc <path>");
}

#[test]
fn cli_short_probes_without_daemon() {
    assert_probe(
        "-V",
        concat!("ck-uc-discussions ", env!("CARGO_PKG_VERSION")),
    );
    assert_probe("-h", "--subc <path>");
}

#[test]
fn cli_ck_domain_probe_without_daemon() {
    assert_probe(
        "--ck-domain",
        "durable multi-party rooms, deliberative councils, and turns",
    );
}

#[test]
fn cli_subcommands_status_and_list() {
    let root = std::env::temp_dir().join(format!("uc-discussions-cli-cmd-{}", uuid::Uuid::new_v4()));
    fs::create_dir(&root).expect("create CLI sandbox");
    let store_path = root.join("test-store.db");

    {
        let storage = uc_discussions::Storage::open_path(&store_path).expect("open test store");
        let service = uc_discussions::service::rooms::RoomsService::new(storage);
        service.create_room(uc_discussions::protocol::rooms::CreateRoomRequest {
            creator: "test-creator".to_owned(),
            topic: "Test Topic".to_owned(),
            goal: "Test Goal".to_owned(),
            stage: "proposal".to_owned(),
        }).expect("create test room");
    }

    let output = std::process::Command::new(env!("CARGO_BIN_EXE_ck-uc-discussions"))
        .args(["--store", store_path.to_str().unwrap(), "status"])
        .output()
        .expect("run status");
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("1 active (1 total)"), "{stdout}");

    let list_output = std::process::Command::new(env!("CARGO_BIN_EXE_ck-uc-discussions"))
        .args(["--store", store_path.to_str().unwrap(), "list"])
        .output()
        .expect("run list");
    assert!(list_output.status.success());
    let list_stdout = String::from_utf8_lossy(&list_output.stdout);
    assert!(list_stdout.contains("Test Topic"), "{list_stdout}");

    fs::remove_dir_all(&root).expect("remove sandbox");
}

#[tokio::test]
async fn health_reports_storage_initialization_failure() {
    // Given a handler whose daemon supplied an invalid storage descriptor.
    let handler = DiscussionsHandler::default();
    handler
        .on_hello_ack(&ModuleHelloAckBody {
            negotiated_ver: 1,
            subc_ops: Vec::new(),
            subc_capabilities: Vec::new(),
            storage: Some(json!({})),
            machine_id: None,
        })
        .await;
    let HandlerOutcome::Error { code, message } = handler.dispatch(b"{}") else {
        panic!("storage initialization failure must reject dispatch");
    };
    assert_eq!(code, "storage_initialization_failed");

    // When health is probed through the module handler interface.
    let health = handler.health().await;

    // Then health reports the same failure that rejects requests.
    assert_eq!(
        health.status,
        HealthStatus::Failing,
        "storage failure must not report healthy"
    );
    assert_eq!(health.detail, Some(message));
}

#[tokio::test]
async fn initialized_health_is_ok() {
    // Given successfully initialized storage.
    let handler = DiscussionsHandler::try_default().expect("initialize storage");

    // When health is probed through the module handler interface.
    let health = handler.health().await;

    // Then the handler is healthy.
    assert_eq!(health.status, HealthStatus::Ok);
}
