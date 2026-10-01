use std::cell::RefCell;
use std::fs::{self, File, OpenOptions};
use std::path::Path;
use std::time::{Duration, Instant};

use anyhow::{Context, Result, bail};
use fs2::FileExt;
use serde_json::{Value, json};

#[derive(Default)]
struct Response {
    json: bool,
    dry_run: bool,
    lines: Vec<String>,
    warnings: Vec<String>,
    data: Value,
    before: Value,
    after: Value,
    lock: Option<File>,
}

thread_local! {
    static RESPONSE: RefCell<Response> = RefCell::new(Response::default());
}

pub fn init(json: bool, dry_run: bool) {
    RESPONSE.with(|state| {
        *state.borrow_mut() = Response {
            json,
            dry_run,
            ..Response::default()
        }
    });
}

pub fn is_json() -> bool {
    RESPONSE.with(|state| state.borrow().json)
}

pub fn is_dry_run() -> bool {
    RESPONSE.with(|state| state.borrow().dry_run)
}

pub fn line(line: String) {
    RESPONSE.with(|state| {
        let mut response = state.borrow_mut();
        if response.json {
            response.lines.push(line);
        } else {
            std::println!("{line}");
        }
    });
}

pub fn data(value: Value) {
    RESPONSE.with(|state| state.borrow_mut().data = value);
}

pub fn warning(line: String) {
    RESPONSE.with(|state| {
        let mut response = state.borrow_mut();
        if response.json {
            response.warnings.push(line);
        } else {
            eprintln!("{line}");
        }
    });
}

pub fn before(value: Value) {
    RESPONSE.with(|state| state.borrow_mut().before = value);
}

pub fn after(value: Value) {
    RESPONSE.with(|state| state.borrow_mut().after = value);
}

pub fn lock(path: &Path) -> Result<()> {
    let parent = path
        .parent()
        .context("config requires a parent directory")?;
    fs::create_dir_all(parent).context("create config directory")?;
    let mut name = path.as_os_str().to_os_string();
    name.push(".lock");
    let file = OpenOptions::new()
        .create(true)
        .truncate(false)
        .read(true)
        .write(true)
        .open(name)
        .context("open config lock")?;
    let start = Instant::now();
    loop {
        match file.try_lock_exclusive() {
            Ok(()) => break,
            Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                if start.elapsed() >= Duration::from_secs(5) {
                    bail!("Config is busy; retry after the other fav process completes");
                }
                std::thread::sleep(Duration::from_millis(25));
            }
            Err(error) => return Err(error).context("lock favorites store"),
        }
    }
    RESPONSE.with(|state| state.borrow_mut().lock = Some(file));
    Ok(())
}

pub fn unlock() {
    RESPONSE.with(|state| state.borrow_mut().lock.take());
}

pub fn finish(command: &str) {
    RESPONSE.with(|state| {
        let response = state.borrow();
        if !response.json { return; }
        let before = response.before["items"].as_array().cloned().unwrap_or_default();
        let after = response.after["items"].as_array().cloned().unwrap_or_else(|| before.clone());
        let mut changed_ids: Vec<u64> = before.iter().chain(after.iter())
            .filter_map(|item| item["id"].as_u64())
            .filter(|id| before.iter().find(|item| item["id"] == *id) != after.iter().find(|item| item["id"] == *id))
            .collect();
        changed_ids.sort_unstable();
        changed_ids.dedup();
        std::println!("{}", json!({
            "schema_version": 1, "ok": true, "command": command,
            "dry_run": response.dry_run, "data": response.data,
            "items": after.iter().filter(|item| changed_ids.contains(&item["id"].as_u64().unwrap_or(0))).collect::<Vec<_>>(),
            "changes": changed_ids.iter().map(|id| json!({"id": id,
                "before": before.iter().find(|item| item["id"] == *id),
                "after": after.iter().find(|item| item["id"] == *id)})).collect::<Vec<_>>(),
            "presets": response.after["presets"],
            "changed_ids": changed_ids, "output": response.lines, "warnings": response.warnings
        }));
    });
}

#[derive(Debug)]
pub struct Failure {
    pub code: &'static str,
    pub exit: i32,
    pub message: String,
}

impl std::fmt::Display for Failure {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.message)
    }
}
impl std::error::Error for Failure {}

pub fn error(error: &anyhow::Error) -> i32 {
    let message = format!("{error:#}");
    let (code, exit) = if let Some(failure) = error.downcast_ref::<Failure>() {
        (failure.code, failure.exit)
    } else if message.contains("No favorite")
        || message.contains("No preset")
        || message.contains("No favorites matched")
    {
        ("not_found", 3)
    } else if message.contains("Already favorited")
        || message.contains("already exists")
        || message.contains("already in use")
        || message.contains("already used")
    {
        ("conflict", 4)
    } else if message.contains("Config is busy") {
        ("busy", 5)
    } else if message.contains("run command")
        || message.contains("run opener")
        || message.contains("Open command failed")
    {
        ("execution_failed", 7)
    } else if message.contains("store")
        || message.contains("directory")
        || message.contains("config lock")
    {
        ("storage_error", 6)
    } else {
        ("invalid_input", 2)
    };
    if is_json() {
        let data = RESPONSE.with(|state| state.borrow().data.clone());
        std::println!(
            "{}",
            json!({"schema_version": 1, "ok": false, "data": data, "error": {
                "code": code, "message": message, "suggestion": match code {
                    "not_found" => "Use fav list --json to discover available targets.",
                    "conflict" => "Use fav ensure for idempotent path registration or choose a unique alias.",
                    "busy" => "Retry after the other process finishes.",
                    "storage_error" => "Check the config path, file permissions, and JSON contents.",
                    "execution_failed" => "Inspect the command output and executable availability.",
                    _ => "Use fav schema or COMMAND --help to inspect valid arguments."
                }
            }})
        );
    } else {
        eprintln!("Error: {message}");
    }
    exit
}
