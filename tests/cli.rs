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

    let id_one = add_with_alias(&config, &file_one, "one", "work");
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

    let with_append = output_fav(
        &["with", &id_one.to_string(), "--", "printf", "%s"],
        &config,
    );
    assert_eq!(
        with_append,
        file_one.canonicalize().unwrap().to_string_lossy()
    );
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
