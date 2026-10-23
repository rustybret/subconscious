#![forbid(unsafe_code)]

use std::{
    env,
    error::Error,
    fs,
    path::{Path, PathBuf},
};

use serde_json::json;

use crate::{
    protocol::{council::GetCouncilRequest, rooms::{GetRoomRequest, PostRoomRequest}},
    service::{council::CouncilService, rooms::RoomsService},
    Storage,
};

pub const DOMAIN_HEADLINE: &str = "durable multi-party rooms, deliberative councils, and turns";

pub const USAGE: &str = "\
ck discussions — inspect and control uc-discussions rooms, councils, and turns

usage: ck discussions <command> [options]

commands:
  status                    inspect discussions daemon and store health
  list                      list active rooms
  room <room_id>            inspect a room: members, stages, outcome, and posts
  council <council_id>      inspect an Athena council deliberation and roster
  post <room_id> <content>  post a turn to an active room

options:
  --subc <path>             daemon connection file
  --store <path>            sqlite store path (default: ~/.local/share/cortexkit/uc-discussions/store.db)
  --author <name>           author for post (default: operator)
  --type <type>             post type (default: position)
  --json                    print raw JSON response
  -V, --version             print version
  -h, --help                print help
";

#[derive(Default, Debug)]
struct ParsedArgs {
    command: Option<String>,
    subcommand_args: Vec<String>,
    store_path: Option<PathBuf>,
    _subc_path: Option<PathBuf>,
    author: Option<String>,
    post_type: Option<String>,
    json: bool,
    ck_domain: bool,
    version: bool,
    help: bool,
}

fn parse_cli_args<I: IntoIterator<Item = String>>(args: I) -> ParsedArgs {
    let mut parsed = ParsedArgs::default();
    let mut iter = args.into_iter().skip(1);

    while let Some(arg) = iter.next() {
        match arg.as_str() {
            "--ck-domain" => parsed.ck_domain = true,
            "-V" | "--version" => parsed.version = true,
            "-h" | "--help" => parsed.help = true,
            "--json" => parsed.json = true,
            "--store" => {
                if let Some(val) = iter.next() {
                    parsed.store_path = Some(PathBuf::from(val));
                }
            }
            "--subc" => {
                if let Some(val) = iter.next() {
                    parsed._subc_path = Some(PathBuf::from(val));
                }
            }
            "--author" => {
                if let Some(val) = iter.next() {
                    parsed.author = Some(val);
                }
            }
            "--type" => {
                if let Some(val) = iter.next() {
                    parsed.post_type = Some(val);
                }
            }
            cmd if !cmd.starts_with('-') && parsed.command.is_none() => {
                parsed.command = Some(cmd.to_owned());
            }
            other => {
                parsed.subcommand_args.push(other.to_owned());
            }
        }
    }
    parsed
}

pub fn default_store_path() -> PathBuf {
    if let Ok(path) = env::var("UC_DISCUSSIONS_STORE") {
        return PathBuf::from(path);
    }
    if let Ok(home) = env::var("HOME") {
        PathBuf::from(home).join(".local/share/cortexkit/uc-discussions/store.db")
    } else {
        PathBuf::from("store.db")
    }
}

pub fn run_cli<I: IntoIterator<Item = String>>(args: I) -> Result<bool, Box<dyn Error>> {
    let parsed = parse_cli_args(args);

    if parsed.ck_domain {
        println!("{DOMAIN_HEADLINE}");
        return Ok(true);
    }

    if parsed.version {
        println!("ck-uc-discussions {}", env!("CARGO_PKG_VERSION"));
        return Ok(true);
    }

    if parsed.help {
        print!("{USAGE}");
        return Ok(true);
    }

    let Some(command) = parsed.command.as_deref() else {
        // No CLI subcommand provided -> supervisor / serve mode
        return Ok(false);
    };

    let store_path = parsed.store_path.unwrap_or_else(default_store_path);

    match command {
        "status" => cmd_status(&store_path, parsed.json)?,
        "list" => cmd_list(&store_path, parsed.json)?,
        "room" => {
            let room_id = parsed.subcommand_args.first().ok_or("room command requires <room_id>")?;
            cmd_room(&store_path, room_id, parsed.json)?;
        }
        "council" => {
            let council_id = parsed.subcommand_args.first().ok_or("council command requires <council_id>")?;
            cmd_council(&store_path, council_id, parsed.json)?;
        }
        "post" => {
            let room_id = parsed.subcommand_args.first().ok_or("post command requires <room_id>")?;
            let content = parsed.subcommand_args.get(1).ok_or("post command requires <content>")?;
            let author = parsed.author.as_deref().unwrap_or("operator");
            let post_type = parsed.post_type.as_deref().unwrap_or("position");
            cmd_post(&store_path, room_id, author, post_type, content, parsed.json)?;
        }
        "help" => {
            print!("{USAGE}");
        }
        unknown => {
            return Err(format!("unknown discussions command '{unknown}'. Run 'ck discussions --help' for usage.").into());
        }
    }

    Ok(true)
}

fn open_storage_checked(path: &Path) -> Result<Storage, Box<dyn Error>> {
    if !path.exists() {
        return Err(format!(
            "discussions store not found at {}.\nEnsure the uc-discussions daemon is running or pass --store <path>.",
            path.display()
        )
        .into());
    }
    Ok(Storage::open_path(path)?)
}

fn cmd_status(path: &Path, json_output: bool) -> Result<(), Box<dyn Error>> {
    let exists = path.exists();
    if !exists {
        if json_output {
            println!("{}", json!({ "status": "not_found", "path": path.display().to_string() }));
        } else {
            println!("store: {} (not found)", path.display());
        }
        return Ok(());
    }

    let storage = Storage::open_path(path)?;
    let connection = storage.lock_connection()?;

    let rooms_total: i64 = connection.query_row("SELECT COUNT(*) FROM rooms", [], |r| r.get(0)).unwrap_or(0);
    let rooms_active: i64 = connection.query_row("SELECT COUNT(*) FROM rooms WHERE status = 'active'", [], |r| r.get(0)).unwrap_or(0);
    let posts_total: i64 = connection.query_row("SELECT COUNT(*) FROM room_posts", [], |r| r.get(0)).unwrap_or(0);
    let councils_total: i64 = connection.query_row("SELECT COUNT(*) FROM council_runs", [], |r| r.get(0)).unwrap_or(0);
    let messages_total: i64 = connection.query_row("SELECT COUNT(*) FROM peer_messages", [], |r| r.get(0)).unwrap_or(0);

    let wal_size = fs::metadata(format!("{}-wal", path.display())).map(|m| m.len()).unwrap_or(0);
    let db_size = fs::metadata(path).map(|m| m.len()).unwrap_or(0);

    if json_output {
        println!("{}", json!({
            "status": "ok",
            "path": path.display().to_string(),
            "size_bytes": db_size,
            "wal_size_bytes": wal_size,
            "rooms_total": rooms_total,
            "rooms_active": rooms_active,
            "posts_total": posts_total,
            "councils_total": councils_total,
            "peer_messages_total": messages_total,
        }));
    } else {
        println!("Store:       {}", path.display());
        println!("Health:      ok (size: {} KB, WAL: {} KB)", db_size / 1024, wal_size / 1024);
        println!("Rooms:       {} active ({} total)", rooms_active, rooms_total);
        println!("Posts:       {}", posts_total);
        println!("Councils:    {}", councils_total);
        println!("Peer DMs:    {}", messages_total);
    }
    Ok(())
}

fn cmd_list(path: &Path, json_output: bool) -> Result<(), Box<dyn Error>> {
    let storage = open_storage_checked(path)?;
    let connection = storage.lock_connection()?;

    let mut stmt = connection.prepare(
        "SELECT room_id, topic, goal, stage, status, created_at FROM rooms ORDER BY created_at DESC LIMIT 50"
    )?;
    let rows = stmt.query_map([], |row| {
        Ok(json!({
            "room_id": row.get::<_, String>(0)?,
            "topic": row.get::<_, String>(1)?,
            "goal": row.get::<_, String>(2)?,
            "stage": row.get::<_, String>(3)?,
            "status": row.get::<_, String>(4)?,
            "created_at": row.get::<_, String>(5)?,
        }))
    })?.collect::<Result<Vec<_>, _>>()?;

    if json_output {
        println!("{}", serde_json::to_string_pretty(&rows)?);
        return Ok(());
    }

    if rows.is_empty() {
        println!("No rooms recorded in store.");
        return Ok(());
    }

    println!("{:<38} {:<10} {:<12} {}", "ROOM ID", "STATUS", "STAGE", "TOPIC");
    println!("{}", "-".repeat(80));
    for r in rows {
        let room_id = r["room_id"].as_str().unwrap_or("-");
        let status = r["status"].as_str().unwrap_or("-");
        let stage = r["stage"].as_str().unwrap_or("-");
        let topic = r["topic"].as_str().unwrap_or("-");
        println!("{:<38} {:<10} {:<12} {}", room_id, status, stage, topic);
    }
    Ok(())
}

fn cmd_room(path: &Path, room_id: &str, json_output: bool) -> Result<(), Box<dyn Error>> {
    let storage = open_storage_checked(path)?;
    let service = RoomsService::new(storage);
    let resp = service.get_room(GetRoomRequest {
        room_id: room_id.to_owned(),
    })?;

    if json_output {
        println!("{}", serde_json::to_string_pretty(&resp)?);
        return Ok(());
    }

    println!("Room:        {}", resp.room_id);
    println!("Topic:       {}", resp.topic);
    println!("Goal:        {}", resp.goal);
    println!("Status:      {} (stage: {})", resp.status, resp.stage);
    println!("Created:     {}", resp.created_at);
    if let Some(closed) = resp.closed_at {
        println!("Closed:      {}", closed);
    }

    if !resp.decisions.is_empty() {
        println!("\nDecisions:");
        for d in &resp.decisions {
            println!("  • {d}");
        }
    }
    if !resp.dissent.is_empty() {
        println!("\nDissent:");
        for d in &resp.dissent {
            println!("  • {d}");
        }
    }

    println!("\nMembers ({}) :", resp.members.len());
    for m in &resp.members {
        let proj = m.project_id.as_deref().unwrap_or("-");
        let sess = m.session_id.as_deref().unwrap_or("-");
        let agent = m.agent.as_deref().unwrap_or("-");
        let inc = m.incarnation.map(|i| i.to_string()).unwrap_or_else(|| "-".to_owned());
        println!("  • {:<16} role={:<10} proj={:<16} sess={:<16} inc={:<3} agent={}", m.member_id, m.role, proj, sess, inc, agent);
    }

    println!("\nPosts ({}) :", resp.posts.len());
    for p in &resp.posts {
        println!("  #{:<3} [{:<10}] {:<16} ({}) : {}", p.seq, p.post_type, p.author, p.created_at, p.content.lines().next().unwrap_or(""));
    }

    Ok(())
}

fn cmd_council(path: &Path, council_id: &str, json_output: bool) -> Result<(), Box<dyn Error>> {
    let storage = open_storage_checked(path)?;
    let service = CouncilService::new(storage);
    let resp = service.get(GetCouncilRequest {
        council_id: council_id.to_owned(),
    })?;

    if json_output {
        println!("{}", serde_json::to_string_pretty(&resp)?);
        return Ok(());
    }

    let c = &resp.council;
    println!("Council:     {}", c.council_id);
    println!("Prompt:      {}", c.question);
    println!("Status:      {}", c.status);
    println!("Started:     {}", c.started_at);
    if let Some(comp) = &c.completed_at {
        println!("Completed:   {}", comp);
    }
    if let Some(synth) = &c.synthesis {
        println!("Synthesis:   {}", synth);
    }
    if let Some(agree) = &c.agreement_level {
        println!("Agreement:   {}", agree);
    }

    println!("\nMembers Roster ({}):", c.member_states.len());
    for m in &c.member_states {
        let err = m.error.as_deref().unwrap_or("-");
        println!("  • {:<18} status={:<12} updated={} error={}", m.member_name, m.status, m.updated_at, err);
    }

    Ok(())
}

fn cmd_post(
    path: &Path,
    room_id: &str,
    author: &str,
    post_type: &str,
    content: &str,
    json_output: bool,
) -> Result<(), Box<dyn Error>> {
    let storage = open_storage_checked(path)?;
    let service = RoomsService::new(storage);
    let resp = service.post(PostRoomRequest {
        room_id: room_id.to_owned(),
        author: author.to_owned(),
        post_type: post_type.to_owned(),
        content: content.to_owned(),
        incarnation: None,
        reply_to_post_id: None,
    })?;

    if json_output {
        println!("{}", serde_json::to_string_pretty(&resp)?);
    } else {
        println!("Turn posted: seq={} post_id={}", resp.seq, resp.post.post_id);
    }
    Ok(())
}
