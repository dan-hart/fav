use std::fs;
use std::path::{Path, PathBuf};

use assert_cmd::cargo::cargo_bin_cmd;
use predicates::prelude::*;
use serde_json::Value;
use tempfile::TempDir;

fn temp_config() -> (TempDir, PathBuf) {
    let dir = tempfile::tempdir().expect("tempdir");
    let config = dir.path().join("fav.json");
    (dir, config)
}

fn run_fav(args: &[&str], config: &Path) -> assert_cmd::assert::Assert {
    let mut cmd = cargo_bin_cmd!("fav");
    cmd.env("FAV_CONFIG", config);
    cmd.args(args);
    cmd.assert()
}

fn output_fav(args: &[&str], config: &Path) -> String {
    let mut cmd = cargo_bin_cmd!("fav");
    let output = cmd
        .env("FAV_CONFIG", config)
        .args(args)
        .output()
        .expect("run");
    assert!(output.status.success(), "command failed: {args:?}");
    String::from_utf8_lossy(&output.stdout).trim().to_string()
}

fn add_with_alias(config: &Path, path: &Path, alias: &str, tag: &str) -> u64 {
    output_fav(
        &[
            "add",
            path.to_str().expect("path"),
            "--alias",
            alias,
            "--tag",
            tag,
            "--id-only",
        ],
        config,
    )
    .parse::<u64>()
    .expect("id")
}

fn list_json(config: &Path) -> Vec<Value> {
    let output = output_fav(&["list", "--format", "json"], config);
    serde_json::from_str(&output).expect("parse json")
}

fn find_by_id(items: &[Value], id: u64) -> Value {
    items
        .iter()
        .find(|item| item.get("id").and_then(|v| v.as_u64()) == Some(id))
        .cloned()
        .expect("item")
}

fn find_by_alias(items: &[Value], alias: &str) -> Value {
    items
        .iter()
        .find(|item| item.get("alias").and_then(|v| v.as_str()) == Some(alias))
        .cloned()
        .expect("item")
}

fn response(args: &[&str], config: &Path) -> Value {
    let mut args = args.to_vec();
    args.insert(0, "--json");
    serde_json::from_str(&output_fav(&args, config)).expect("response JSON")
}

#[test]
fn schema_is_available_without_config_and_describes_global_flags() {
    let (dir, config) = temp_config();
    let schema: Value = serde_json::from_str(&output_fav(&["schema"], &config)).unwrap();
    assert_eq!(schema["schema_version"], 1);
    assert!(
        schema["cli"]["commands"]
            .as_array()
            .unwrap()
            .iter()
            .any(|v| v["name"] == "ensure")
    );
    assert!(
        schema["cli"]["arguments"]
            .as_array()
            .unwrap()
            .iter()
            .any(|v| v["long"] == "non-interactive")
    );
    assert_eq!(fs::read_dir(dir.path()).unwrap().count(), 0);
}

#[test]
fn ensure_retries_preserve_id_usage_and_unsupplied_metadata() {
    let (dir, config) = temp_config();
    let path = dir.path().join("notes with spaces.txt");
    fs::write(&path, "notes").unwrap();
    let first = response(
        &[
            "ensure",
            path.to_str().unwrap(),
            "--alias",
            "notes",
            "--note",
            "Keep me",
            "--tag",
            "active",
        ],
        &config,
    );
    let second = response(
        &["ensure", path.to_str().unwrap(), "--tag", "active"],
        &config,
    );
    assert_eq!(first["data"]["id"], second["data"]["id"]);
    assert_eq!(second["changed_ids"], serde_json::json!([]));
    assert_eq!(second["data"]["note"], "Keep me");
    assert_eq!(second["data"]["uses"], 0);
}

#[test]
fn resolve_and_no_touch_preserve_config_bytes() {
    let (dir, config) = temp_config();
    add_with_alias(&config, dir.path(), "project", "active");
    let before = fs::read(&config).unwrap();
    let result = response(&["resolve", "project"], &config);
    assert_eq!(
        result["data"]["path"],
        dir.path()
            .canonicalize()
            .unwrap()
            .to_string_lossy()
            .as_ref()
    );
    response(&["get", "project", "--no-touch"], &config);
    response(&["project", "--no-touch"], &config);
    assert_eq!(before, fs::read(&config).unwrap());
}

#[test]
fn mutation_previews_report_proposals_without_writing() {
    let (dir, config) = temp_config();
    add_with_alias(&config, dir.path(), "project", "active");
    let before = fs::read(&config).unwrap();
    let meta = response(
        &[
            "meta",
            "--query",
            "alias:=project",
            "--note",
            "New note",
            "--dry-run",
        ],
        &config,
    );
    assert_eq!(meta["dry_run"], true);
    assert_eq!(meta["changes"][0]["after"]["note"], "New note");
    let removed = response(&["rm", "--query", "alias:=project", "--dry-run"], &config);
    assert_eq!(removed["changes"][0]["before"]["alias"], "project");
    assert!(removed["changes"][0]["after"].is_null());
    assert_eq!(before, fs::read(&config).unwrap());
}

#[test]
fn preview_import_prune_and_export_do_not_write() {
    let (dir, config) = temp_config();
    let path = dir.path().join("missing-soon");
    fs::write(&path, "test").unwrap();
    add_with_alias(&config, &path, "missing", "test");
    fs::remove_file(&path).unwrap();
    let before = fs::read(&config).unwrap();
    let pruned = response(&["health", "--prune", "--dry-run"], &config);
    assert_eq!(pruned["changed_ids"].as_array().unwrap().len(), 1);
    let paths = dir.path().join("paths.json");
    fs::write(
        &paths,
        serde_json::to_vec(&vec![dir.path().to_str().unwrap()]).unwrap(),
    )
    .unwrap();
    let imported = response(
        &[
            "import",
            "paths",
            "--file",
            paths.to_str().unwrap(),
            "--dry-run",
        ],
        &config,
    );
    assert_eq!(imported["changed_ids"].as_array().unwrap().len(), 1);
    let export = dir.path().join("export.json");
    response(
        &[
            "io",
            "--export",
            "--file",
            export.to_str().unwrap(),
            "--dry-run",
        ],
        &config,
    );
    assert!(!export.exists());
    assert_eq!(before, fs::read(&config).unwrap());
}

#[test]
fn quoted_exact_queries_and_negation_target_only_expected_rows() {
    let (dir, config) = temp_config();
    response(
        &[
            "ensure",
            dir.path().to_str().unwrap(),
            "--alias",
            "project",
            "--note",
            "Release checklist",
        ],
        &config,
    );
    let other = dir.path().join("other");
    fs::create_dir(&other).unwrap();
    response(
        &[
            "ensure",
            other.to_str().unwrap(),
            "--alias",
            "project-old",
            "--note",
            "Release checklist draft",
        ],
        &config,
    );
    let result = response(
        &[
            "list",
            "--query",
            "note:=\"release checklist\" -alias:=project-old",
        ],
        &config,
    );
    assert_eq!(result["data"].as_array().unwrap().len(), 1);
    assert_eq!(result["data"][0]["alias"], "project");
    for query in ["", "   ", "-", "note:\"unterminated", "alias:=", "id:nope"] {
        run_fav(&["--json", "rm", "--query", query, "--yes"], &config).code(2);
    }
    assert_eq!(list_json(&config).len(), 3);
}

#[test]
fn json_errors_and_interactive_refusal_are_machine_readable() {
    let (_dir, config) = temp_config();
    let mut cmd = cargo_bin_cmd!("fav");
    let output = cmd
        .env("FAV_CONFIG", &config)
        .args(["get", "999", "--json"])
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(3));
    let error: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(error["error"]["code"], "not_found");
    assert!(
        error["error"]["suggestion"]
            .as_str()
            .unwrap()
            .contains("list")
    );
    for command in ["pick", "tui"] {
        run_fav(&[command, "--non-interactive"], &config)
            .code(2)
            .stderr(predicate::str::contains("disabled"));
        run_fav(&[command, "--json"], &config).code(2);
    }
    let mut cmd = cargo_bin_cmd!("fav");
    let output = cmd
        .env("FAV_CONFIG", &config)
        .args(["list", "--bogus", "--json"])
        .output()
        .unwrap();
    assert_eq!(
        serde_json::from_slice::<Value>(&output.stdout).unwrap()["error"]["code"],
        "invalid_arguments"
    );
}

#[test]
fn concurrent_additions_do_not_lose_updates() {
    let (dir, config) = temp_config();
    let mut children = Vec::new();
    for index in 0..12 {
        let path = dir.path().join(format!("file-{index}"));
        fs::write(&path, "data").unwrap();
        let mut cmd = std::process::Command::new(assert_cmd::cargo::cargo_bin!("fav"));
        children.push(
            cmd.env("FAV_CONFIG", &config)
                .arg("ensure")
                .arg(path)
                .arg("--json")
                .stdout(std::process::Stdio::null())
                .stderr(std::process::Stdio::piped())
                .spawn()
                .unwrap(),
        );
    }
    for child in children {
        let output = child.wait_with_output().unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
    }
    assert_eq!(list_json(&config).len(), 13);
    assert!(!config.with_extension("json.tmp").exists());
}

#[test]
fn unusual_paths_are_preserved() {
    let (dir, config) = temp_config();
    for name in ["-leading-hyphen", "space name", "caf\u{e9}-\u{1f680}"] {
        let path = dir.path().join(name);
        fs::write(&path, "ok").unwrap();
        let added = response(&["ensure", path.to_str().unwrap()], &config);
        let id = added["data"]["id"].to_string();
        let resolved = response(&["resolve", &id], &config);
        assert_eq!(
            resolved["data"]["path"],
            path.canonicalize().unwrap().to_string_lossy().as_ref()
        );
    }
}

#[cfg(unix)]
#[test]
fn symlink_ensure_uses_canonical_identity() {
    let (dir, config) = temp_config();
    let target = dir.path().join("target");
    let link = dir.path().join("link");
    fs::write(&target, "ok").unwrap();
    std::os::unix::fs::symlink(&target, &link).unwrap();
    let first = response(&["ensure", target.to_str().unwrap()], &config);
    let second = response(&["ensure", link.to_str().unwrap()], &config);
    assert_eq!(first["data"]["id"], second["data"]["id"]);
}

#[cfg(unix)]
#[test]
fn json_execution_captures_output_and_reports_child_failure() {
    let (dir, config) = temp_config();
    add_with_alias(&config, dir.path(), "project", "active");
    let preview = response(
        &["with", "project", "--dry-run", "--", "printf", "%s", "{}"],
        &config,
    );
    assert_eq!(preview["changed_ids"], serde_json::json!([]));
    assert_eq!(preview["data"]["program"], "printf");
    let result = response(&["with", "project", "--", "printf", "%s", "{}"], &config);
    assert_eq!(result["data"]["exit_code"], 0);
    assert_eq!(
        result["data"]["stdout"],
        dir.path()
            .canonicalize()
            .unwrap()
            .to_string_lossy()
            .as_ref()
    );
    run_fav(
        &["with", "project", "--json", "--", "sh", "-c", "exit 9"],
        &config,
    )
    .code(7)
    .stdout(predicate::str::contains("execution_failed"));
}

#[test]
fn add_list_alias_tag_flow() {
    let (_dir, config) = temp_config();
    let data_dir = tempfile::tempdir().expect("data dir");
    let file_path = data_dir.path().join("file.txt");
    fs::write(&file_path, "hello").expect("write");

    let id = output_fav(&["add", file_path.to_str().unwrap(), "--id-only"], &config)
        .parse::<u64>()
        .expect("id");
    assert_eq!(id, 2);

    run_fav(&["meta", "2", "--alias", "myfile"], &config).success();
    run_fav(&["meta", "2", "--tag", "work"], &config).success();

    let items = list_json(&config);
    let item = find_by_id(&items, 2);
    assert_eq!(item.get("alias").and_then(|v| v.as_str()), Some("myfile"));
    assert!(
        item.get("tags")
            .and_then(|v| v.as_array())
            .unwrap()
            .iter()
            .any(|tag| tag == "work")
    );

    let resolved = output_fav(&["myfile"], &config);
    let canonical = file_path.canonicalize().expect("canonical");
    assert_eq!(resolved, canonical.to_string_lossy());
}

#[test]
fn search_and_get() {
    let (_dir, config) = temp_config();
    let data_dir = tempfile::tempdir().expect("data dir");
    let file_path = data_dir.path().join("notes.txt");
    fs::write(&file_path, "hello").expect("write");

    let id = output_fav(&["add", file_path.to_str().unwrap(), "--id-only"], &config)
        .parse::<u64>()
        .expect("id");

    let search = output_fav(&["list", "--search", "notes", "--format", "json"], &config);
    let items: Vec<Value> = serde_json::from_str(&search).expect("json");
    let item = find_by_id(&items, id);
    assert_eq!(item.get("id").and_then(|v| v.as_u64()), Some(id));

    let got = output_fav(
        &["get", &id.to_string(), "--path-format", "absolute"],
        &config,
    );
    assert_eq!(got, file_path.canonicalize().unwrap().to_string_lossy());
}

#[test]
fn export_import_roundtrip() {
    let (_dir, config) = temp_config();
    let data_dir = tempfile::tempdir().expect("data dir");
    let file_path = data_dir.path().join("file.txt");
    fs::write(&file_path, "hello").expect("write");

    output_fav(&["add", file_path.to_str().unwrap(), "--id-only"], &config);

    let export_file = data_dir.path().join("export.json");
    run_fav(
        &["io", "--export", "--file", export_file.to_str().unwrap()],
        &config,
    )
    .success();

    let (_dir2, config2) = temp_config();
    run_fav(
        &["io", "--import", "--file", export_file.to_str().unwrap()],
        &config2,
    )
    .success();

    let items = list_json(&config2);
    assert!(items.len() >= 2);
}

#[test]
fn check_and_prune_missing() {
    let (_dir, config) = temp_config();
    let data_dir = tempfile::tempdir().expect("data dir");
    let file_path = data_dir.path().join("gone.txt");
    fs::write(&file_path, "hello").expect("write");

    let id = output_fav(&["add", file_path.to_str().unwrap(), "--id-only"], &config)
        .parse::<u64>()
        .expect("id");
    fs::remove_file(&file_path).expect("remove");

    let check_out = output_fav(&["health", "--path-format", "absolute"], &config);
    assert!(check_out.contains(&id.to_string()));

    let pruned = output_fav(&["health", "--prune"], &config);
    assert_eq!(pruned, "1");
}

#[test]
#[cfg(unix)]
fn with_command_inserts_path() {
    let (_dir, config) = temp_config();
    let data_dir = tempfile::tempdir().expect("data dir");
    let file_path = data_dir.path().join("file.txt");
    fs::write(&file_path, "hello").expect("write");

    let id = output_fav(&["add", file_path.to_str().unwrap(), "--id-only"], &config)
        .parse::<u64>()
        .expect("id");

    let output = output_fav(
        &["with", &id.to_string(), "--", "printf", "%s", "{}"],
        &config,
    );
    assert_eq!(output, file_path.canonicalize().unwrap().to_string_lossy());
}

#[cfg(unix)]
#[test]
fn config_symlink_survives_atomic_updates() {
    let (dir, config) = temp_config();
    let file = dir.path().join("target.txt");
    fs::write(&file, "test").unwrap();
    response(
        &["ensure", file.to_str().unwrap(), "--alias", "first"],
        &config,
    );
    let link = dir.path().join("config-link.json");
    std::os::unix::fs::symlink(&config, &link).unwrap();
    response(
        &["ensure", file.to_str().unwrap(), "--alias", "second"],
        &link,
    );
    assert!(
        fs::symlink_metadata(&link)
            .unwrap()
            .file_type()
            .is_symlink()
    );
    assert_eq!(
        response(&["resolve", "second"], &config)["data"]["path"],
        file.canonicalize().unwrap().to_string_lossy().as_ref()
    );
}

#[test]
fn malformed_preset_is_rejected_without_panic_or_write() {
    let (_dir, config) = temp_config();
    let contents =
        r#"{"version":2,"next_id":1,"items":[],"presets":[{"name":"bad","command":[]}]}"#;
    fs::write(&config, contents).unwrap();
    run_fav(&["--json", "preset", "list"], &config)
        .code(6)
        .stdout(predicate::str::contains("storage_error"));
    assert_eq!(fs::read_to_string(&config).unwrap(), contents);
}

#[test]
fn list_formats() {
    let (_dir, config) = temp_config();
    let data_dir = tempfile::tempdir().expect("data dir");
    let file_one = data_dir.path().join("one.txt");
    let file_two = data_dir.path().join("two.txt");
    let file_three = data_dir.path().join("three.txt");
    fs::write(&file_one, "one").expect("write");
    fs::write(&file_two, "two").expect("write");
    fs::write(&file_three, "three").expect("write");

    let _id_one = add_with_alias(&config, &file_one, "one", "work");
    let _id_two = add_with_alias(&config, &file_two, "two", "ops");
    let _id_three = add_with_alias(&config, &file_three, "three", "work");

    let table = output_fav(&["list"], &config);
    assert!(table.contains("one"));

    let plain = output_fav(&["list", "--format", "plain"], &config);
    assert!(plain.contains('\t'));

    let json = output_fav(&["list", "--format", "json"], &config);
    let items: Vec<Value> = serde_json::from_str(&json).expect("list json");
    assert!(items.iter().any(|item| {
        item.get("tags")
            .and_then(|v| v.as_array())
            .map(|tags| tags.iter().any(|tag| tag == "work"))
            .unwrap_or(false)
    }));
}

#[test]
fn meta_tag_mutations() {
    let (_dir, config) = temp_config();
    let data_dir = tempfile::tempdir().expect("data dir");
    let file_path = data_dir.path().join("alpha.txt");
    fs::write(&file_path, "alpha").expect("write");

    let id = add_with_alias(&config, &file_path, "alpha", "work");

    run_fav(
        &["meta", &id.to_string(), "--rename-tag", "work=ops"],
        &config,
    )
    .success();
    let item = find_by_id(&list_json(&config), id);
    assert!(
        item.get("tags")
            .and_then(|v| v.as_array())
            .unwrap()
            .iter()
            .any(|tag| tag == "ops")
    );

    run_fav(&["meta", &id.to_string(), "--rm-tag", "ops"], &config).success();
    let item = find_by_id(&list_json(&config), id);
    assert!(
        item.get("tags")
            .and_then(|v| v.as_array())
            .unwrap()
            .is_empty()
    );

    run_fav(
        &["meta", &id.to_string(), "--tag", "new", "--set-tags"],
        &config,
    )
    .success();
    let item = find_by_id(&list_json(&config), id);
    assert!(
        item.get("tags")
            .and_then(|v| v.as_array())
            .unwrap()
            .iter()
            .any(|tag| tag == "new")
    );

    run_fav(&["meta", &id.to_string(), "--alias", "beta"], &config).success();
    run_fav(&["meta", &id.to_string(), "--clear-alias"], &config).success();
    let item = find_by_id(&list_json(&config), id);
    assert!(item.get("alias").is_none() || item.get("alias").unwrap().is_null());
}

#[test]
fn errors_and_remove() {
    let (_dir, config) = temp_config();
    let data_dir = tempfile::tempdir().expect("data dir");
    let file_path = data_dir.path().join("dupe.txt");
    fs::write(&file_path, "dupe").expect("write");

    let id = add_with_alias(&config, &file_path, "alpha", "work");

    run_fav(&["add", file_path.to_str().unwrap()], &config).failure();
    run_fav(&["meta", &id.to_string(), "--alias", "list"], &config).failure();
    run_fav(&["meta", &id.to_string(), "--alias", "123"], &config).failure();
    run_fav(&["meta", &id.to_string()], &config).failure();
    run_fav(
        &["meta", &id.to_string(), "--tag", "work", "--rm-tag", "ops"],
        &config,
    )
    .failure();

    run_fav(&["rm", &id.to_string()], &config).success();
    let items = list_json(&config);
    assert!(
        !items
            .iter()
            .any(|item| item.get("id").and_then(|v| v.as_u64()) == Some(id))
    );
}

#[test]
fn export_import_merge_and_with_append() {
    let (_dir, config) = temp_config();
    let data_dir = tempfile::tempdir().expect("data dir");
    let file_one = data_dir.path().join("one.txt");
    let file_two = data_dir.path().join("two.txt");
    fs::write(&file_one, "one").expect("write");
    fs::write(&file_two, "two").expect("write");

    let _id_one = add_with_alias(&config, &file_one, "one", "work");
    let _id_two = add_with_alias(&config, &file_two, "two", "ops");

    let export_stdout = output_fav(&["io", "--export"], &config);
    let export_value: Value = serde_json::from_str(&export_stdout).expect("export json");
    assert!(
        export_value
            .get("items")
            .and_then(|v| v.as_array())
            .unwrap()
            .len()
            >= 3
    );

    let export_file = data_dir.path().join("export.json");
    run_fav(
        &["io", "--export", "--file", export_file.to_str().unwrap()],
        &config,
    )
    .success();

    let (_dir2, config2) = temp_config();
    let file_three = data_dir.path().join("three.txt");
    fs::write(&file_three, "three").expect("write");
    add_with_alias(&config2, &file_three, "three", "misc");

    run_fav(
        &[
            "io",
            "--import",
            "--file",
            export_file.to_str().unwrap(),
            "--merge",
        ],
        &config2,
    )
    .success();
    let merged = list_json(&config2);
    assert!(merged.len() >= 4);

    #[cfg(unix)]
    {
        let with_append = output_fav(
            &["with", &_id_one.to_string(), "--", "printf", "%s"],
            &config,
        );
        assert_eq!(
            with_append,
            file_one.canonicalize().unwrap().to_string_lossy()
        );
    }
}

#[test]
fn list_no_matches_prints_help() {
    let (_dir, config) = temp_config();
    run_fav(&["list", "--tag", "missing"], &config).success();
    run_fav(&["list", "--search", "missing"], &config).success();
}

#[test]
fn add_without_id_only_outputs_message() {
    let (_dir, config) = temp_config();
    let data_dir = tempfile::tempdir().expect("data dir");
    let file_path = data_dir.path().join("plain.txt");
    fs::write(&file_path, "plain").expect("write");

    let output = output_fav(&["add", file_path.to_str().unwrap()], &config);
    assert!(output.contains("Added favorite"));
}

#[test]
fn notes_query_and_batch_updates_work() {
    let (_dir, config) = temp_config();
    let data_dir = tempfile::tempdir().expect("data dir");
    let file_one = data_dir.path().join("alpha.txt");
    let file_two = data_dir.path().join("beta.txt");
    fs::write(&file_one, "alpha").expect("write");
    fs::write(&file_two, "beta").expect("write");

    let alpha_id = output_fav(
        &[
            "add",
            file_one.to_str().unwrap(),
            "--alias",
            "alpha",
            "--tag",
            "work",
            "--note",
            "Primary project notes",
            "--id-only",
        ],
        &config,
    )
    .parse::<u64>()
    .expect("id");

    output_fav(
        &[
            "add",
            file_two.to_str().unwrap(),
            "--alias",
            "beta",
            "--tag",
            "archive",
            "--id-only",
        ],
        &config,
    );

    let queried = output_fav(
        &["list", "--query", "note:project", "--format", "json"],
        &config,
    );
    let queried_items: Vec<Value> = serde_json::from_str(&queried).expect("query json");
    assert_eq!(
        find_by_id(&queried_items, alpha_id).get("alias").unwrap(),
        "alpha"
    );

    run_fav(
        &[
            "meta",
            "--query",
            "tag:archive",
            "--note",
            "Cold storage",
            "--yes",
        ],
        &config,
    )
    .success();
    let items = list_json(&config);
    let beta = find_by_alias(&items, "beta");
    assert_eq!(
        beta.get("note").and_then(|v| v.as_str()),
        Some("Cold storage")
    );

    run_fav(&["rm", "--query", "tag:archive", "--yes"], &config).success();
    let items = list_json(&config);
    assert!(
        items
            .iter()
            .all(|item| item.get("alias").and_then(|v| v.as_str()) != Some("beta"))
    );
}

#[test]
fn meta_can_clear_notes() {
    let (_dir, config) = temp_config();
    let data_dir = tempfile::tempdir().expect("data dir");
    let file_path = data_dir.path().join("noted.txt");
    fs::write(&file_path, "noted").expect("write");

    let id = output_fav(
        &[
            "add",
            file_path.to_str().unwrap(),
            "--alias",
            "noted",
            "--note",
            "Needs cleanup",
            "--id-only",
        ],
        &config,
    )
    .parse::<u64>()
    .expect("id");

    run_fav(&["meta", &id.to_string(), "--clear-note"], &config).success();
    let items = list_json(&config);
    let item = find_by_id(&items, id);
    assert!(item.get("note").is_none() || item.get("note").unwrap().is_null());
}

#[test]
fn shell_open_and_presets_work() {
    let (_dir, config) = temp_config();
    let data_dir = tempfile::tempdir().expect("data dir");
    let file_path = data_dir.path().join("open.txt");
    fs::write(&file_path, "open").expect("write");

    let id = output_fav(
        &[
            "add",
            file_path.to_str().unwrap(),
            "--alias",
            "openit",
            "--id-only",
        ],
        &config,
    )
    .parse::<u64>()
    .expect("id");

    let shell = output_fav(&["shell", "init", "zsh"], &config);
    assert!(shell.contains("fcd()"));
    assert!(shell.contains("fav preset run"));

    let open_cmd = output_fav(&["open", &id.to_string(), "--dry-run"], &config);
    assert!(open_cmd.contains(file_path.canonicalize().unwrap().to_string_lossy().as_ref()));

    run_fav(
        &[
            "preset",
            "add",
            "show",
            "--note",
            "Show the selected path",
            "--",
            "printf",
            "%s",
            "{}",
        ],
        &config,
    )
    .success();
    let preset_list = output_fav(&["preset", "list"], &config);
    assert!(preset_list.contains("show"));

    let preset_run = output_fav(&["preset", "run", "show", "openit", "--dry-run"], &config);
    assert!(preset_run.contains("printf"));
    assert!(preset_run.contains(file_path.canonicalize().unwrap().to_string_lossy().as_ref()));

    run_fav(&["preset", "rm", "show"], &config).success();
}

#[test]
fn doctor_duplicates_and_repair_dry_run_work() {
    let (_dir, config) = temp_config();
    let data_dir = tempfile::tempdir().expect("data dir");
    let dup_path = data_dir.path().join("dup.txt");
    fs::write(&dup_path, "dup").expect("write");
    let dup_path = dup_path.canonicalize().expect("canonicalize");

    let config_json = serde_json::json!({
        "version": 2,
        "next_id": 4,
        "items": [
            {"id": 1, "path": dup_path, "alias": "one", "tags": [], "note": null, "uses": 0, "last_used": null},
            {"id": 2, "path": dup_path, "alias": "two", "tags": [], "note": null, "uses": 0, "last_used": null}
        ],
        "presets": []
    });
    fs::write(&config, serde_json::to_vec(&config_json).expect("json")).expect("write config");

    let duplicates = output_fav(&["doctor", "duplicates"], &config);
    assert!(duplicates.contains("path"));
    assert!(duplicates.contains(dup_path.to_string_lossy().as_ref()));

    let old_root = data_dir.path().join("old");
    let new_root = data_dir.path().join("new");
    fs::create_dir_all(&new_root).expect("new root");
    let new_file = new_root.join("moved.txt");
    fs::write(&new_file, "moved").expect("new file");
    let old_file = old_root.join("moved.txt");

    let repair_json = serde_json::json!({
        "version": 2,
        "next_id": 3,
        "items": [
            {"id": 1, "path": old_file, "alias": "moved", "tags": [], "note": null, "uses": 0, "last_used": null}
        ],
        "presets": []
    });
    fs::write(&config, serde_json::to_vec(&repair_json).expect("json")).expect("write config");

    let repair = output_fav(
        &[
            "doctor",
            "repair",
            "--from",
            old_root.to_str().unwrap(),
            "--to",
            new_root.to_str().unwrap(),
            "--dry-run",
        ],
        &config,
    );
    assert!(repair.contains("moved"));
    assert!(repair.contains(new_file.canonicalize().unwrap().to_string_lossy().as_ref()));
}

#[test]
fn importers_and_with_dry_run_work() {
    let (_dir, config) = temp_config();
    let data_dir = tempfile::tempdir().expect("data dir");
    let hist_dir = data_dir.path().join("hist");
    let json_dir = data_dir.path().join("json");
    fs::create_dir_all(&hist_dir).expect("hist dir");
    fs::create_dir_all(&json_dir).expect("json dir");

    let hist_target = hist_dir.canonicalize().expect("canonicalize hist");
    let path_target = json_dir.canonicalize().expect("canonicalize json");

    let history_file = data_dir.path().join("zsh_history");
    fs::write(
        &history_file,
        format!(": 1710000000:0;cd {}\n", hist_target.to_string_lossy()),
    )
    .expect("write history");

    run_fav(
        &[
            "import",
            "history",
            "--shell",
            "zsh",
            "--file",
            history_file.to_str().unwrap(),
            "--tag",
            "history",
        ],
        &config,
    )
    .success();

    let paths_file = data_dir.path().join("paths.json");
    fs::write(
        &paths_file,
        serde_json::to_vec(&vec![path_target.to_string_lossy().to_string()]).expect("json"),
    )
    .expect("write paths");

    run_fav(
        &[
            "import",
            "paths",
            "--file",
            paths_file.to_str().unwrap(),
            "--note",
            "Imported from file",
        ],
        &config,
    )
    .success();

    let items = list_json(&config);
    assert!(items.iter().any(|item| {
        item.get("path").and_then(|v| v.as_str()) == Some(hist_target.to_string_lossy().as_ref())
    }));
    assert!(
        items.iter().any(|item| {
            item.get("note").and_then(|v| v.as_str()) == Some("Imported from file")
        })
    );

    let with_dry_run = output_fav(&["with", "1", "--dry-run", "--", "printf", "%s"], &config);
    assert!(with_dry_run.contains("printf"));
}

#[test]
fn import_from_stdin_merge() {
    let (_dir, config) = temp_config();
    let data_dir = tempfile::tempdir().expect("data dir");
    let file_path = data_dir.path().join("stdin.txt");
    fs::write(&file_path, "stdin").expect("write");
    add_with_alias(&config, &file_path, "stdin", "work");

    let exported = output_fav(&["io", "--export"], &config);

    let (_dir2, config2) = temp_config();
    let mut cmd = cargo_bin_cmd!("fav");
    cmd.env("FAV_CONFIG", &config2)
        .args(["io", "--import", "--merge"])
        .write_stdin(exported)
        .assert()
        .success();
    let items = list_json(&config2);
    assert!(items.len() >= 2);
}

#[test]
fn no_args_and_help_are_handled() {
    let (_dir, config) = temp_config();
    let _ = output_fav(&[], &config);
    run_fav(&["--help"], &config).success();
}

#[test]
fn tag_set_without_tags_errors() {
    let (_dir, config) = temp_config();
    let data_dir = tempfile::tempdir().expect("data dir");
    let file_path = data_dir.path().join("tagset.txt");
    fs::write(&file_path, "tagset").expect("write");

    let id = add_with_alias(&config, &file_path, "tagset", "work");
    run_fav(&["meta", &id.to_string(), "--set-tags"], &config).failure();
}

#[test]
fn get_missing_id_includes_hint() {
    let (_dir, config) = temp_config();
    run_fav(&["get", "999"], &config)
        .failure()
        .stderr(predicate::str::contains("No favorite with id 999"));
}

#[test]
fn add_missing_path_includes_hint() {
    let (_dir, config) = temp_config();
    let tmp = tempfile::tempdir().expect("tempdir");
    let missing_path = tmp.path().join("missing.txt");
    run_fav(&["add", missing_path.to_str().unwrap()], &config)
        .failure()
        .stderr(predicate::str::contains("Path does not exist"));
}

#[test]
fn import_empty_stdin_errors() {
    let (_dir, config) = temp_config();
    let mut cmd = cargo_bin_cmd!("fav");
    cmd.env("FAV_CONFIG", &config)
        .args(["io", "--import"])
        .write_stdin("")
        .assert()
        .failure()
        .stderr(predicate::str::contains("No input provided"));
}

#[cfg(feature = "coverage")]
#[test]
fn pick_and_tui_no_output_under_coverage() {
    let (_dir, config) = temp_config();
    let data_dir = tempfile::tempdir().expect("data dir");
    let file_path = data_dir.path().join("pick.txt");
    fs::write(&file_path, "pick").expect("write");

    add_with_alias(&config, &file_path, "picked", "work");

    let pick_out = output_fav(&["pick", "--tag", "work"], &config);
    assert_eq!(
        pick_out,
        file_path.canonicalize().unwrap().to_string_lossy()
    );

    let tui_out = output_fav(&["tui", "--tag", "work"], &config);
    assert_eq!(tui_out, file_path.canonicalize().unwrap().to_string_lossy());
}
