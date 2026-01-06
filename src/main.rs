use std::env;
use std::fs;
use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};
use std::process::Command as ProcessCommand;
#[cfg(not(feature = "coverage"))]
use std::process::Stdio;
#[cfg(not(feature = "coverage"))]
use std::time::Duration;
use std::time::{SystemTime, UNIX_EPOCH};

use anyhow::{Context, Result, bail};
use clap::{Args, Parser, Subcommand, ValueEnum};
#[cfg(not(feature = "coverage"))]
use crossterm::{
    cursor::{Hide, Show},
    event::{self, Event, KeyCode, KeyEvent, KeyModifiers},
    execute,
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};
use directories::UserDirs;
#[cfg(not(feature = "coverage"))]
use ratatui::{
    Terminal,
    backend::CrosstermBackend,
    layout::{Constraint, Direction, Layout},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, ListState, Paragraph},
};
use serde::{Deserialize, Serialize};

const STORE_VERSION: u32 = 1;

#[derive(Parser)]
#[command(
    name = "fav",
    version,
    about = "Keep track of favorite files and folders",
    long_about = "fav keeps a small, local database of favorite files and folders.\n\
Use numeric ids (speed-dial) or aliases to print paths quickly.\n\
Examples:\n  fav add\n  fav add ~/dotfiles --alias dotfiles --tag config\n  fav list --tag work --sort uses\n  fav list --search notes --format json\n  fav 1\n  fav my-config\n  fav with dotfiles -- rg \"TODO\" {}\n"
)]
struct Cli {
    /// Path to the favorites config (defaults to ~/.fav.config)
    #[arg(long, global = true)]
    config: Option<PathBuf>,
    #[command(subcommand)]
    command: Option<Command>,
}

#[derive(Subcommand)]
enum Command {
    #[command(
        about = "Add a favorite",
        long_about = "Add a favorite file or directory.\n\
If no path is provided, the current directory is added.\n\
Use --id-only for script-friendly output.\n\
Examples:\n  fav add\n  fav add ./notes/todo.md --alias todo --tag notes\n  fav add ~/dotfiles --alias dotfiles --tag config --tag work\n  fav add --id-only\n"
    )]
    Add(AddArgs),
    #[command(
        about = "List favorites",
        long_about = "List all favorites.\n\
You can filter by tag and/or search term.\n\
Examples:\n  fav\n  fav list --tag work\n  fav list --search notes --format json\n  fav list --path-format relative --sort uses\n"
    )]
    List(ListArgs),
    #[command(
        about = "Print a favorite path",
        long_about = "Print the path for a favorite by id or alias.\n\
This is shell-friendly output (path only).\n\
Examples:\n  fav get 1\n  fav get dotfiles\n  cd \"$(fav get dotfiles)\"\n  fav get 1 --path-format relative\n"
    )]
    Get(GetArgs),
    #[command(
        about = "Update metadata",
        long_about = "Update alias or tags for a favorite.\n\
The target can be an id, alias, or path.\n\
Examples:\n  fav meta 1 --alias dotfiles\n  fav meta 1 --clear-alias\n  fav meta 1 --tag work,urgent\n  fav meta 1 --set-tags work,urgent\n  fav meta 1 --rm-tag urgent\n  fav meta 1 --rename-tag old=new\n"
    )]
    Meta(MetaArgs),
    #[command(
        about = "Import or export favorites",
        long_about = "Export favorites to JSON or import them from JSON.\n\
If no file is provided, export writes to stdout and import reads from stdin.\n\
Examples:\n  fav io --export > backup.json\n  fav io --export --file backup.json\n  fav io --import --file backup.json\n  cat backup.json | fav io --import --merge\n"
    )]
    Io(IoArgs),
    #[command(
        about = "Check or prune missing paths",
        long_about = "List favorites whose paths no longer exist, or prune them.\n\
Examples:\n  fav health\n  fav health --path-format absolute\n  fav health --prune\n"
    )]
    Health(HealthArgs),
    #[command(
        about = "Interactively pick a favorite",
        long_about = "Pick a favorite via fzf and print its path.\n\
Requires `fzf` to be installed.\n\
Examples:\n  fav pick\n  fav pick --tag work\n  fav pick --path-format absolute --display-path-format relative\n"
    )]
    Pick(PickArgs),
    #[command(
        about = "Browse favorites in a TUI",
        long_about = "Browse favorites in a terminal UI and print the selected path.\n\
Type to filter, use arrows to move, Enter to select, Esc/Ctrl+C to quit.\n\
Examples:\n  fav tui\n  fav tui --tag work\n  fav tui --path-format absolute --display-path-format relative\n"
    )]
    Tui(TuiArgs),
    #[command(
        about = "Run a command with a favorite path",
        long_about = "Run a command using a favorite path.\n\
If any argument contains {}, it will be replaced with the path.\n\
Otherwise the path is appended to the end of the command.\n\
Examples:\n  fav with my-config -- cat\n  fav with 3 -- ls -la {}\n  fav with notes -- rg \"TODO\" {}\n"
    )]
    With(WithArgs),
    #[command(
        aliases = ["remove", "delete", "del"],
        about = "Remove a favorite",
        long_about = "Remove a favorite by id, alias, or path.\n\
Examples:\n  fav rm 1\n  fav rm dotfiles\n  fav rm ~/dotfiles\n  fav rm ./notes/todo.md\n"
    )]
    Rm(RemoveArgs),
}

#[derive(Args)]
struct AddArgs {
    /// Path to favorite (defaults to current directory)
    path: Option<PathBuf>,
    /// Optional alias for the favorite (must be unique)
    #[arg(short, long)]
    alias: Option<String>,
    /// Tags to apply (repeatable or comma-separated)
    #[arg(short, long, value_delimiter = ',')]
    tag: Vec<String>,
    /// Print only the id (script-friendly)
    #[arg(long)]
    id_only: bool,
}

#[derive(Args)]
struct ListArgs {
    /// Filter by tag (repeatable; all tags must match)
    #[arg(long)]
    tag: Vec<String>,
    /// Search query (matches alias/path/tags)
    #[arg(long)]
    search: Option<String>,
    /// Output format
    #[arg(long, value_enum, default_value = "table")]
    format: OutputFormat,
    /// How to render paths
    #[arg(long, value_enum, default_value = "relative")]
    path_format: PathFormat,
    /// Sort by field
    #[arg(long, value_enum, default_value = "id")]
    sort: SortBy,
    /// Reverse sort order
    #[arg(long)]
    reverse: bool,
}

#[derive(Args)]
struct GetArgs {
    /// Target id or alias
    target: String,
    /// How to render paths
    #[arg(long, value_enum, default_value = "absolute")]
    path_format: PathFormat,
}

#[derive(Args)]
struct MetaArgs {
    /// Target id/alias/path
    target: String,
    /// Set or replace alias
    #[arg(long)]
    alias: Option<String>,
    /// Clear alias
    #[arg(long)]
    clear_alias: bool,
    /// Tags to add (repeatable or comma-separated)
    #[arg(long, value_delimiter = ',')]
    tag: Vec<String>,
    /// Replace existing tags instead of appending
    #[arg(long)]
    set_tags: bool,
    /// Remove tags (repeatable or comma-separated)
    #[arg(long, value_delimiter = ',')]
    rm_tag: Vec<String>,
    /// Rename a tag (format: old=new)
    #[arg(long)]
    rename_tag: Option<String>,
}

#[derive(Args)]
struct PickArgs {
    /// Filter by tag (repeatable; all tags must match)
    #[arg(long)]
    tag: Vec<String>,
    /// Search query (matches alias/path/tags)
    #[arg(long)]
    search: Option<String>,
    /// How to render output paths
    #[arg(long, value_enum, default_value = "absolute")]
    path_format: PathFormat,
    /// How to render paths in the picker display
    #[arg(long, value_enum, default_value = "relative")]
    display_path_format: PathFormat,
    /// Sort by field
    #[arg(long, value_enum, default_value = "id")]
    sort: SortBy,
    /// Reverse sort order
    #[arg(long)]
    reverse: bool,
}

#[derive(Args)]
struct TuiArgs {
    /// Filter by tag (repeatable; all tags must match)
    #[arg(long)]
    tag: Vec<String>,
    /// Initial search query (matches alias/path/tags)
    #[arg(long)]
    search: Option<String>,
    /// How to render output paths
    #[arg(long, value_enum, default_value = "absolute")]
    path_format: PathFormat,
    /// How to render paths in the TUI display
    #[arg(long, value_enum, default_value = "relative")]
    display_path_format: PathFormat,
    /// Sort by field
    #[arg(long, value_enum, default_value = "id")]
    sort: SortBy,
    /// Reverse sort order
    #[arg(long)]
    reverse: bool,
}

#[derive(Args)]
struct IoArgs {
    /// Export favorites to JSON
    #[arg(long)]
    export: bool,
    /// Import favorites from JSON
    #[arg(long)]
    import: bool,
    /// File to read/write (defaults to stdin/stdout)
    #[arg(long)]
    file: Option<PathBuf>,
    /// Merge into existing favorites instead of replacing (import only)
    #[arg(long)]
    merge: bool,
}

#[derive(Args)]
struct HealthArgs {
    /// Prune missing favorites instead of listing them
    #[arg(long)]
    prune: bool,
    /// How to render paths
    #[arg(long, value_enum, default_value = "relative")]
    path_format: PathFormat,
}

#[derive(Args)]
struct WithArgs {
    /// Target id/alias/path
    target: String,
    /// Command to run (use -- to separate)
    #[arg(required = true, trailing_var_arg = true, allow_hyphen_values = true)]
    command: Vec<String>,
    /// How to render paths
    #[arg(long, value_enum, default_value = "absolute")]
    path_format: PathFormat,
}

#[derive(Args)]
struct RemoveArgs {
    /// Target id/alias/path
    target: String,
}

#[derive(Clone, Copy, Debug, ValueEnum)]
enum OutputFormat {
    Table,
    Plain,
    Json,
}

#[derive(Clone, Copy, Debug, ValueEnum)]
enum PathFormat {
    Tilde,
    Absolute,
    Relative,
}

#[derive(Clone, Copy, Debug, ValueEnum)]
enum SortBy {
    Id,
    Alias,
    Path,
    Tag,
    Recent,
    Uses,
}

#[derive(Debug, Serialize, Deserialize, Default)]
struct Store {
    version: u32,
    next_id: u64,
    items: Vec<Favorite>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
struct Favorite {
    id: u64,
    path: String,
    alias: Option<String>,
    tags: Vec<String>,
    #[serde(default)]
    uses: u64,
    #[serde(default)]
    last_used: Option<i64>,
}

fn main() -> Result<()> {
    let args: Vec<String> = env::args().collect();

    if let Some(result) = try_handle_alias_or_get(&args)? {
        println!("{result}");
        return Ok(());
    }

    let cli = Cli::parse();
    let store_path = resolve_config_path(cli.config.as_deref())?;
    let mut store = load_store(&store_path)?;

    match cli.command.unwrap_or(Command::List(ListArgs {
        tag: Vec::new(),
        search: None,
        format: OutputFormat::Table,
        path_format: PathFormat::Relative,
        sort: SortBy::Id,
        reverse: false,
    })) {
        Command::Add(args) => {
            let path = args
                .path
                .unwrap_or(env::current_dir().context("get current directory")?);
            let normalized = normalize_path(&path)?;
            if let Some(existing) = store
                .items
                .iter()
                .find(|item| item.path == normalized.to_string_lossy())
            {
                bail!(
                    "Already favorited as id {}{}",
                    existing.id,
                    existing
                        .alias
                        .as_ref()
                        .map(|a| format!(" (alias: {a})"))
                        .unwrap_or_default()
                );
            }

            if let Some(alias) = args.alias.as_ref() {
                validate_alias(alias)?;
                ensure_unique_alias(&store, alias)?;
            }

            let mut tags = normalize_tags(args.tag);
            tags.sort();
            tags.dedup();

            let id = store.next_id.max(1);
            store.next_id = id + 1;
            store.items.push(Favorite {
                id,
                path: normalized.to_string_lossy().to_string(),
                alias: args.alias,
                tags,
                uses: 0,
                last_used: None,
            });
            store.items.sort_by_key(|item| item.id);
            save_store(&store_path, &store)?;
            if args.id_only {
                println!("{id}");
            } else {
                let item = store.items.last().context("missing added favorite")?;
                let alias = item.alias.as_deref().unwrap_or("-");
                let tags = if item.tags.is_empty() {
                    "-".to_string()
                } else {
                    item.tags.join(",")
                };
                println!(
                    "Added favorite {id}: {} (alias: {alias}, tags: {tags})",
                    format_path(item.path.as_str(), PathFormat::Relative)?
                );
            }
        }
        Command::List(args) => {
            let filters = Filters {
                search: args.search.clone(),
                tags: args.tag.clone(),
            };
            let opts = ListOptions {
                format: args.format,
                path_format: args.path_format,
                sort: args.sort,
                reverse: args.reverse,
            };
            let count = list_items(&store, &filters, &opts)?;
            if count == 0 {
                eprintln!("No favorites matched. Try: fav add, fav list --search <term>");
            }
        }
        Command::Get(args) => {
            let idx = resolve_target_index(&store, &args.target)?;
            mark_used(&mut store.items[idx]);
            let output = format_path(store.items[idx].path.as_str(), args.path_format)?;
            save_store(&store_path, &store)?;
            println!("{output}");
        }
        Command::Meta(args) => {
            let idx = resolve_target_index(&store, &args.target)?;
            let has_alias = args.alias.is_some();
            let has_clear_alias = args.clear_alias;
            let has_add_tags = !args.tag.is_empty();
            let has_rm_tags = !args.rm_tag.is_empty();
            let has_rename_tag = args.rename_tag.is_some();
            let tag_actions = has_add_tags as u8 + has_rm_tags as u8 + has_rename_tag as u8;
            if has_alias && has_clear_alias {
                bail!("Choose only one of: --alias or --clear-alias");
            }
            if tag_actions > 1 {
                bail!("Choose only one of: --tag, --rm-tag, or --rename-tag");
            }
            if args.set_tags && !has_add_tags {
                bail!("--set-tags can only be used with --tag");
            }
            if !has_alias && !has_clear_alias && tag_actions == 0 {
                bail!("Provide metadata to update (alias or tags)");
            }

            let mut alias_updated = false;
            let mut tags_updated = false;

            if has_clear_alias {
                store.items[idx].alias = None;
                alias_updated = true;
            } else if let Some(alias) = args.alias.as_deref() {
                validate_alias(alias)?;
                if store.items[idx].alias.as_deref() != Some(alias) {
                    ensure_unique_alias(&store, alias)?;
                    store.items[idx].alias = Some(alias.to_string());
                    alias_updated = true;
                }
            }

            if let Some(rename) = args.rename_tag.as_deref() {
                let (old, new) = parse_rename(rename)?;
                let tags = &mut store.items[idx].tags;
                for tag in tags.iter_mut() {
                    if tag.eq_ignore_ascii_case(old) {
                        *tag = new.to_string();
                    }
                }
                tags.sort();
                tags.dedup();
                tags_updated = true;
            } else if has_rm_tags {
                let remove = normalize_tags(args.rm_tag);
                store.items[idx]
                    .tags
                    .retain(|tag| !remove.iter().any(|rm| rm.eq_ignore_ascii_case(tag)));
                tags_updated = true;
            } else if has_add_tags {
                let mut tags = normalize_tags(args.tag);
                if args.set_tags {
                    store.items[idx].tags = tags;
                } else {
                    store.items[idx].tags.append(&mut tags);
                    store.items[idx].tags.sort();
                    store.items[idx].tags.dedup();
                }
                tags_updated = true;
            }

            save_store(&store_path, &store)?;
            let item = &store.items[idx];
            if alias_updated && tags_updated {
                println!("Updated alias and tags for {}.", item.id);
            } else if alias_updated {
                println!(
                    "Updated alias for {}: {}",
                    item.id,
                    item.alias.as_deref().unwrap_or("-")
                );
            } else if tags_updated {
                let tags = if item.tags.is_empty() {
                    "-".to_string()
                } else {
                    item.tags.join(",")
                };
                println!("Updated tags for {}: {tags}", item.id);
            }
        }
        Command::Pick(args) => {
            let filters = Filters {
                search: args.search.clone(),
                tags: args.tag.clone(),
            };
            let opts = ListOptions {
                format: OutputFormat::Plain,
                path_format: args.display_path_format,
                sort: args.sort,
                reverse: args.reverse,
            };
            let selection = pick_item(&store, &filters, &opts)?;
            if let Some(id) = selection {
                let idx = store
                    .items
                    .iter()
                    .position(|item| item.id == id)
                    .context("No favorite with that id")?;
                mark_used(&mut store.items[idx]);
                let output = format_path(store.items[idx].path.as_str(), args.path_format)?;
                save_store(&store_path, &store)?;
                println!("{output}");
            }
        }
        Command::Tui(args) => {
            let selection = run_tui(&store, &args)?;
            if let Some(id) = selection {
                let idx = store
                    .items
                    .iter()
                    .position(|item| item.id == id)
                    .context("No favorite with that id")?;
                mark_used(&mut store.items[idx]);
                let output = format_path(store.items[idx].path.as_str(), args.path_format)?;
                save_store(&store_path, &store)?;
                println!("{output}");
            }
        }
        Command::Io(args) => {
            if args.export == args.import {
                bail!("Choose exactly one of --export or --import");
            }
            if args.merge && !args.import {
                bail!("--merge can only be used with --import");
            }
            if args.export {
                let has_file = args.file.is_some();
                export_store(&store, args.file)?;
                if has_file {
                    eprintln!("Exported {} favorites.", store.items.len());
                }
            } else {
                let imported = import_store(args.file)?;
                if args.merge {
                    merge_store(&mut store, imported)?;
                } else {
                    store = imported;
                }
                sync_store(&mut store);
                save_store(&store_path, &store)?;
                eprintln!("Imported {} favorites.", store.items.len());
            }
        }
        Command::Health(args) => {
            if args.prune {
                let removed = prune_missing(&mut store)?;
                save_store(&store_path, &store)?;
                println!("{removed}");
            } else {
                check_missing(&store, args.path_format)?;
            }
        }
        Command::With(args) => {
            let idx = resolve_target_index(&store, &args.target)?;
            mark_used(&mut store.items[idx]);
            let path = format_path(store.items[idx].path.as_str(), args.path_format)?;
            save_store(&store_path, &store)?;
            run_with_command(&args.command, &path)?;
        }
        Command::Rm(args) => {
            let idx = resolve_target_index(&store, &args.target)?;
            store.items.remove(idx);
            save_store(&store_path, &store)?;
            println!("Removed favorite.");
        }
    }

    Ok(())
}

fn try_handle_alias_or_get(args: &[String]) -> Result<Option<String>> {
    if args.len() < 2 {
        return Ok(None);
    }

    let mut override_config: Option<PathBuf> = None;
    let mut positional: Vec<&str> = Vec::new();
    let mut skip_next = false;
    for (index, arg) in args.iter().enumerate().skip(1) {
        if skip_next {
            skip_next = false;
            continue;
        }
        if arg == "--config" {
            let next = args.get(index + 1).context("Missing value for --config")?;
            override_config = Some(PathBuf::from(next));
            skip_next = true;
            continue;
        }
        if let Some(rest) = arg.strip_prefix("--config=") {
            override_config = Some(PathBuf::from(rest));
            continue;
        }
        if arg.starts_with('-') {
            return Ok(None);
        }
        positional.push(arg.as_str());
    }

    if positional.len() != 1 {
        return Ok(None);
    }
    let first = positional[0];
    if is_reserved_word(first) {
        return Ok(None);
    }

    let store_path = resolve_config_path(override_config.as_deref())?;
    let mut store = load_store(&store_path)?;

    if let Ok(id) = first.parse::<u64>() {
        let idx = store
            .items
            .iter()
            .position(|item| item.id == id)
            .context("No favorite with that id")?;
        mark_used(&mut store.items[idx]);
        let output = format_path(store.items[idx].path.as_str(), PathFormat::Absolute)?;
        save_store(&store_path, &store)?;
        return Ok(Some(output));
    }

    let idx = store
        .items
        .iter()
        .position(|item| item.alias.as_deref() == Some(first))
        .context("No favorite with that alias")?;
    mark_used(&mut store.items[idx]);
    let output = format_path(store.items[idx].path.as_str(), PathFormat::Absolute)?;
    save_store(&store_path, &store)?;
    Ok(Some(output))
}

fn is_reserved_word(value: &str) -> bool {
    matches!(
        value,
        "add"
            | "list"
            | "get"
            | "meta"
            | "io"
            | "health"
            | "pick"
            | "tui"
            | "with"
            | "rm"
            | "remove"
            | "delete"
            | "del"
            | "help"
            | "version"
    )
}

fn load_store(store_path: &Path) -> Result<Store> {
    if !store_path.exists() {
        let seed_path = store_path.display().to_string();
        return Ok(Store {
            version: STORE_VERSION,
            next_id: 2,
            items: vec![Favorite {
                id: 1,
                path: seed_path,
                alias: None,
                tags: Vec::new(),
                uses: 0,
                last_used: None,
            }],
        });
    }

    let data = fs::read_to_string(store_path).context("read favorites store")?;
    let mut store: Store = serde_json::from_str(&data).context("parse favorites store")?;
    sync_store(&mut store);
    Ok(store)
}

fn save_store(path: &Path, store: &Store) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).context("create data directory")?;
    }
    let tmp_path = path.with_extension("json.tmp");
    let data = serde_json::to_vec(store).context("serialize favorites store")?;
    fs::write(&tmp_path, data).context("write favorites store")?;
    fs::rename(&tmp_path, path).context("save favorites store")?;
    Ok(())
}

fn resolve_config_path(override_path: Option<&Path>) -> Result<PathBuf> {
    if let Some(path) = override_path {
        return expand_and_absolute(path);
    }
    if let Some(env_path) = env::var_os("FAV_CONFIG") {
        return expand_and_absolute(Path::new(&env_path));
    }
    let user_dirs = UserDirs::new().context("resolve home directory")?;
    Ok(user_dirs.home_dir().join(".fav.config"))
}

fn sync_store(store: &mut Store) {
    if store.version == 0 {
        store.version = STORE_VERSION;
    }
    let max_id = store.items.iter().map(|item| item.id).max().unwrap_or(0);
    if store.next_id <= max_id {
        store.next_id = max_id + 1;
    }
    store.items.sort_by_key(|item| item.id);
}

struct Filters {
    search: Option<String>,
    tags: Vec<String>,
}

struct ListOptions {
    format: OutputFormat,
    path_format: PathFormat,
    sort: SortBy,
    reverse: bool,
}

#[derive(Serialize)]
struct OutputItem {
    id: u64,
    alias: Option<String>,
    path: String,
    tags: Vec<String>,
    uses: u64,
    last_used: Option<i64>,
}

fn list_items(store: &Store, filters: &Filters, opts: &ListOptions) -> Result<usize> {
    let mut items = filtered_items(store, filters);
    sort_items(&mut items, opts.sort, opts.reverse);
    let count = items.len();

    match opts.format {
        OutputFormat::Table => {
            for item in &items {
                let alias = item.alias.as_deref().unwrap_or("-");
                let tags = if item.tags.is_empty() {
                    "-".to_string()
                } else {
                    item.tags.join(",")
                };
                let path = format_path(item.path.as_str(), opts.path_format)?;
                println!("{:>4}  {:<20}  {:<54}  {}", item.id, alias, path, tags);
            }
        }
        OutputFormat::Plain => {
            for item in &items {
                let alias = item.alias.as_deref().unwrap_or("-");
                let tags = if item.tags.is_empty() {
                    "-".to_string()
                } else {
                    item.tags.join(",")
                };
                let path = format_path(item.path.as_str(), opts.path_format)?;
                println!("{}\t{}\t{}\t{}", item.id, alias, path, tags);
            }
        }
        OutputFormat::Json => {
            let output: Vec<OutputItem> = items
                .iter()
                .map(|item| OutputItem {
                    id: item.id,
                    alias: item.alias.clone(),
                    path: format_path(item.path.as_str(), opts.path_format)
                        .unwrap_or_else(|_| item.path.clone()),
                    tags: item.tags.clone(),
                    uses: item.uses,
                    last_used: item.last_used,
                })
                .collect();
            let data = serde_json::to_vec(&output).context("serialize output")?;
            io::stdout().write_all(&data).context("write output")?;
        }
    }
    Ok(count)
}

fn filtered_items<'a>(store: &'a Store, filters: &Filters) -> Vec<&'a Favorite> {
    let query = filters.search.as_ref().map(|q| q.to_lowercase());
    store
        .items
        .iter()
        .filter(|item| {
            if !filters.tags.is_empty() {
                let has_all = filters
                    .tags
                    .iter()
                    .all(|tag| item.tags.iter().any(|t| t.eq_ignore_ascii_case(tag)));
                if !has_all {
                    return false;
                }
            }
            if let Some(q) = query.as_ref()
                && !matches_query(item, q)
            {
                return false;
            }
            true
        })
        .collect()
}

fn matches_query(item: &Favorite, query: &str) -> bool {
    item.alias
        .as_ref()
        .is_some_and(|alias| alias.to_lowercase().contains(query))
        || item.path.to_lowercase().contains(query)
        || item
            .tags
            .iter()
            .any(|tag| tag.to_lowercase().contains(query))
}

fn sort_items(items: &mut Vec<&Favorite>, sort: SortBy, reverse: bool) {
    match sort {
        SortBy::Id => items.sort_by_key(|item| item.id),
        SortBy::Alias => items.sort_by_key(|item| item.alias.clone().unwrap_or_default()),
        SortBy::Path => items.sort_by_key(|item| item.path.clone()),
        SortBy::Tag => items.sort_by_key(|item| item.tags.first().cloned().unwrap_or_default()),
        SortBy::Recent => items.sort_by_key(|item| item.last_used.unwrap_or(0)),
        SortBy::Uses => items.sort_by_key(|item| item.uses),
    }
    let should_reverse = reverse || matches!(sort, SortBy::Recent | SortBy::Uses);
    if should_reverse {
        items.reverse();
    }
}

fn mark_used(item: &mut Favorite) {
    item.uses = item.uses.saturating_add(1);
    item.last_used = Some(now_timestamp());
}

fn now_timestamp() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|dur| dur.as_secs() as i64)
        .unwrap_or(0)
}

fn parse_rename(value: &str) -> Result<(&str, &str)> {
    let mut parts = value.splitn(2, '=');
    let old = parts.next().unwrap_or_default().trim();
    let new = parts.next().unwrap_or_default().trim();
    if old.is_empty() || new.is_empty() {
        bail!("Rename must be in the form old=new");
    }
    Ok((old, new))
}

#[cfg(not(feature = "coverage"))]
fn pick_item(store: &Store, filters: &Filters, opts: &ListOptions) -> Result<Option<u64>> {
    let mut items = filtered_items(store, filters);
    sort_items(&mut items, opts.sort, opts.reverse);
    if items.is_empty() {
        return Ok(None);
    }

    let mut child = ProcessCommand::new("fzf")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .context("launch fzf (is it installed?)")?;

    {
        let stdin = child.stdin.as_mut().context("open fzf stdin")?;
        for item in items.iter() {
            let alias = item.alias.as_deref().unwrap_or("-");
            let tags = if item.tags.is_empty() {
                "-".to_string()
            } else {
                item.tags.join(",")
            };
            let path = format_path(item.path.as_str(), opts.path_format)?;
            writeln!(stdin, "{}\t{}\t{}\t{}", item.id, alias, path, tags)
                .context("write to fzf")?;
        }
    }

    let output = child.wait_with_output().context("wait for fzf")?;
    if !output.status.success() {
        return Ok(None);
    }
    let selection = String::from_utf8_lossy(&output.stdout);
    let selected_line = selection.lines().next().unwrap_or("");
    if selected_line.is_empty() {
        return Ok(None);
    }
    let id_str = selected_line.split_whitespace().next().unwrap_or("");
    let id = id_str.parse::<u64>().context("parse selected id")?;
    Ok(Some(id))
}

#[cfg(feature = "coverage")]
fn pick_item(store: &Store, filters: &Filters, opts: &ListOptions) -> Result<Option<u64>> {
    let mut items = filtered_items(store, filters);
    sort_items(&mut items, opts.sort, opts.reverse);
    Ok(items.first().map(|item| item.id))
}

#[cfg(not(feature = "coverage"))]
struct TuiCleanup;

#[cfg(not(feature = "coverage"))]
impl Drop for TuiCleanup {
    fn drop(&mut self) {
        let _ = disable_raw_mode();
        let mut stdout = io::stdout();
        let _ = execute!(stdout, LeaveAlternateScreen, Show);
    }
}

#[cfg(not(feature = "coverage"))]
fn run_tui(store: &Store, args: &TuiArgs) -> Result<Option<u64>> {
    let base_filters = Filters {
        search: args.search.clone(),
        tags: args.tag.clone(),
    };
    let list_opts = ListOptions {
        format: OutputFormat::Plain,
        path_format: args.display_path_format,
        sort: args.sort,
        reverse: args.reverse,
    };

    enable_raw_mode().context("enable raw mode")?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, Hide).context("enter alternate screen")?;
    let _cleanup = TuiCleanup;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend).context("create terminal")?;

    let mut filter = String::new();
    let mut selected_index: usize = 0;

    loop {
        let mut items = filtered_items(store, &base_filters);
        if !filter.is_empty() {
            let filter_lower = filter.to_lowercase();
            items.retain(|item| matches_query(item, filter_lower.as_str()));
        }
        sort_items(&mut items, list_opts.sort, list_opts.reverse);

        if items.is_empty() {
            selected_index = 0;
        } else if selected_index >= items.len() {
            selected_index = items.len() - 1;
        }

        terminal.draw(|frame| {
            let size = frame.size();
            let sections = Layout::default()
                .direction(Direction::Vertical)
                .constraints([
                    Constraint::Length(3),
                    Constraint::Min(3),
                    Constraint::Length(2),
                ])
                .split(size);

            let base_search = args.search.as_deref().unwrap_or("-");
            let tags = if args.tag.is_empty() {
                "-".to_string()
            } else {
                args.tag.join(",")
            };
            let header = Paragraph::new(Line::from(vec![
                Span::styled("Filter: ", Style::default().add_modifier(Modifier::BOLD)),
                Span::raw(&filter),
                Span::raw("  "),
                Span::styled("base:", Style::default().add_modifier(Modifier::BOLD)),
                Span::raw(format!(" {base_search}  ")),
                Span::styled("tag:", Style::default().add_modifier(Modifier::BOLD)),
                Span::raw(format!(" {tags}")),
            ]))
            .block(Block::default().borders(Borders::ALL).title("fav tui"));
            frame.render_widget(header, sections[0]);

            let list_items: Vec<ListItem> = items
                .iter()
                .map(|item| {
                    let alias = item.alias.as_deref().unwrap_or("-");
                    let tags = if item.tags.is_empty() {
                        "-".to_string()
                    } else {
                        item.tags.join(",")
                    };
                    let path = format_path(item.path.as_str(), args.display_path_format)
                        .unwrap_or_else(|_| item.path.clone());
                    let line =
                        Line::from(format!("{:>4}  {:<20}  {}  {}", item.id, alias, path, tags));
                    ListItem::new(line)
                })
                .collect();

            let mut state = ListState::default();
            if !items.is_empty() {
                state.select(Some(selected_index));
            }

            let list = List::new(list_items)
                .block(Block::default().borders(Borders::ALL).title("favorites"))
                .highlight_style(Style::default().add_modifier(Modifier::REVERSED));
            frame.render_stateful_widget(list, sections[1], &mut state);

            let footer = Paragraph::new(Line::from(
                "Enter: select  Esc/Ctrl+C: quit  ↑/↓: move  Backspace: delete  Ctrl+u: clear",
            ));
            frame.render_widget(footer, sections[2]);
        })?;

        if event::poll(Duration::from_millis(200)).context("poll input")?
            && let Event::Key(KeyEvent {
                code, modifiers, ..
            }) = event::read().context("read input")?
        {
            match (code, modifiers) {
                (KeyCode::Esc, _) => return Ok(None),
                (KeyCode::Char('c'), KeyModifiers::CONTROL) => return Ok(None),
                (KeyCode::Char('u'), KeyModifiers::CONTROL) => {
                    filter.clear();
                    selected_index = 0;
                }
                (KeyCode::Up, _) => {
                    selected_index = selected_index.saturating_sub(1);
                }
                (KeyCode::Down, _) => {
                    selected_index = selected_index.saturating_add(1);
                }
                (KeyCode::Home, _) => selected_index = 0,
                (KeyCode::End, _) => selected_index = usize::MAX,
                (KeyCode::Backspace, _) => {
                    filter.pop();
                    selected_index = 0;
                }
                (KeyCode::Enter, _) => {
                    let mut items = filtered_items(store, &base_filters);
                    if !filter.is_empty() {
                        let filter_lower = filter.to_lowercase();
                        items.retain(|item| matches_query(item, filter_lower.as_str()));
                    }
                    sort_items(&mut items, list_opts.sort, list_opts.reverse);
                    if items.is_empty() {
                        continue;
                    }
                    let idx = selected_index.min(items.len() - 1);
                    return Ok(Some(items[idx].id));
                }
                (KeyCode::Char(ch), KeyModifiers::NONE | KeyModifiers::SHIFT) => {
                    filter.push(ch);
                    selected_index = 0;
                }
                _ => {}
            }
        }
    }
}

#[cfg(feature = "coverage")]
fn run_tui(store: &Store, args: &TuiArgs) -> Result<Option<u64>> {
    let filters = Filters {
        search: args.search.clone(),
        tags: args.tag.clone(),
    };
    let opts = ListOptions {
        format: OutputFormat::Plain,
        path_format: args.display_path_format,
        sort: args.sort,
        reverse: args.reverse,
    };
    let mut items = filtered_items(store, &filters);
    sort_items(&mut items, opts.sort, opts.reverse);
    Ok(items.first().map(|item| item.id))
}

fn export_store(store: &Store, file: Option<PathBuf>) -> Result<()> {
    let data = serde_json::to_vec_pretty(store).context("serialize store")?;
    match file {
        Some(path) => {
            if let Some(parent) = path.parent()
                && !parent.as_os_str().is_empty()
            {
                fs::create_dir_all(parent)
                    .with_context(|| format!("create {}", parent.display()))?;
            }
            fs::write(&path, data).with_context(|| format!("write {}", path.display()))?;
        }
        None => {
            io::stdout().write_all(&data).context("write export")?;
        }
    }
    Ok(())
}

fn import_store(file: Option<PathBuf>) -> Result<Store> {
    let mut data = String::new();
    match file {
        Some(path) => {
            data = fs::read_to_string(&path).with_context(|| format!("read {}", path.display()))?;
        }
        None => {
            io::stdin()
                .read_to_string(&mut data)
                .context("read stdin")?;
        }
    }
    let mut store: Store = serde_json::from_str(&data).context("parse store")?;
    sync_store(&mut store);
    Ok(store)
}

fn merge_store(existing: &mut Store, mut imported: Store) -> Result<()> {
    let aliases: Vec<String> = existing
        .items
        .iter()
        .filter_map(|item| item.alias.clone())
        .collect();
    for item in imported.items.iter() {
        if let Some(alias) = item.alias.as_ref()
            && aliases.iter().any(|a| a == alias)
        {
            bail!("Alias conflict during merge: {alias}");
        }
    }
    let mut next_id = existing.next_id.max(1);
    for mut item in imported.items.drain(..) {
        item.id = next_id;
        next_id += 1;
        existing.items.push(item);
    }
    existing.next_id = next_id;
    existing.items.sort_by_key(|item| item.id);
    Ok(())
}

fn check_missing(store: &Store, format: PathFormat) -> Result<()> {
    for item in store.items.iter() {
        if !Path::new(item.path.as_str()).exists() {
            let path = format_path(item.path.as_str(), format)?;
            println!(
                "{}\t{}\t{}",
                item.id,
                item.alias.as_deref().unwrap_or("-"),
                path
            );
        }
    }
    Ok(())
}

fn prune_missing(store: &mut Store) -> Result<usize> {
    let before = store.items.len();
    store
        .items
        .retain(|item| Path::new(item.path.as_str()).exists());
    let removed = before.saturating_sub(store.items.len());
    Ok(removed)
}

fn run_with_command(command: &[String], path: &str) -> Result<()> {
    let mut cmd = ProcessCommand::new(command.first().context("Provide a command after --")?);
    let mut args: Vec<String> = Vec::new();
    let mut replaced = false;
    for arg in command.iter().skip(1) {
        if arg.contains("{}") {
            args.push(arg.replace("{}", path));
            replaced = true;
        } else {
            args.push(arg.clone());
        }
    }
    if !replaced {
        args.push(path.to_string());
    }
    let status = cmd.args(args).status().context("run command")?;
    let code = status.code().unwrap_or(1);
    std::process::exit(code);
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    use tempfile::tempdir;

    fn sample_store() -> Store {
        Store {
            version: STORE_VERSION,
            next_id: 4,
            items: vec![
                Favorite {
                    id: 1,
                    path: "/tmp/alpha".to_string(),
                    alias: Some("alpha".to_string()),
                    tags: vec!["work".to_string(), "notes".to_string()],
                    uses: 2,
                    last_used: Some(100),
                },
                Favorite {
                    id: 2,
                    path: "/var/log/syslog".to_string(),
                    alias: None,
                    tags: vec!["ops".to_string()],
                    uses: 5,
                    last_used: Some(200),
                },
                Favorite {
                    id: 3,
                    path: "/home/user/docs".to_string(),
                    alias: Some("docs".to_string()),
                    tags: vec!["work".to_string()],
                    uses: 1,
                    last_used: Some(50),
                },
            ],
        }
    }

    #[test]
    fn filters_by_tag() {
        let store = sample_store();
        let filters = Filters {
            search: None,
            tags: vec!["work".to_string()],
        };
        let items = filtered_items(&store, &filters);
        let ids: Vec<u64> = items.into_iter().map(|item| item.id).collect();
        assert_eq!(ids, vec![1, 3]);
    }

    #[test]
    fn filters_by_search() {
        let store = sample_store();
        let filters = Filters {
            search: Some("sys".to_string()),
            tags: Vec::new(),
        };
        let items = filtered_items(&store, &filters);
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].id, 2);
    }

    #[test]
    fn sort_by_recent_desc() {
        let store = sample_store();
        let mut items: Vec<&Favorite> = store.items.iter().collect();
        sort_items(&mut items, SortBy::Recent, false);
        let ids: Vec<u64> = items.into_iter().map(|item| item.id).collect();
        assert_eq!(ids, vec![2, 1, 3]);
    }

    #[test]
    fn sort_by_alias_asc() {
        let store = sample_store();
        let mut items: Vec<&Favorite> = store.items.iter().collect();
        sort_items(&mut items, SortBy::Alias, false);
        let ids: Vec<u64> = items.into_iter().map(|item| item.id).collect();
        assert_eq!(ids, vec![2, 1, 3]);
    }

    #[test]
    fn normalize_tags_splits_and_trims() {
        let tags = normalize_tags(vec![
            "alpha".to_string(),
            "beta,gamma".to_string(),
            "  delta ".to_string(),
            "".to_string(),
        ]);
        assert_eq!(tags, vec!["alpha", "beta", "gamma", "delta"]);
    }

    #[test]
    fn parse_rename_valid_and_invalid() {
        let (old, new) = parse_rename("old=new").expect("rename");
        assert_eq!(old, "old");
        assert_eq!(new, "new");
        assert!(parse_rename("missing").is_err());
    }

    #[test]
    fn sync_store_sets_next_id() {
        let mut store = Store {
            version: 0,
            next_id: 0,
            items: vec![Favorite {
                id: 7,
                path: "/tmp/x".to_string(),
                alias: None,
                tags: Vec::new(),
                uses: 0,
                last_used: None,
            }],
        };
        sync_store(&mut store);
        assert_eq!(store.version, STORE_VERSION);
        assert_eq!(store.next_id, 8);
    }

    #[test]
    fn merge_store_assigns_new_ids() {
        let mut existing = sample_store();
        let imported = Store {
            version: STORE_VERSION,
            next_id: 1,
            items: vec![Favorite {
                id: 1,
                path: "/tmp/other".to_string(),
                alias: Some("unique".to_string()),
                tags: vec!["x".to_string()],
                uses: 0,
                last_used: None,
            }],
        };
        merge_store(&mut existing, imported).expect("merge");
        assert_eq!(existing.items.len(), 4);
        assert_eq!(existing.items.last().unwrap().id, 4);
    }

    #[test]
    fn merge_store_rejects_alias_conflict() {
        let mut existing = sample_store();
        let imported = Store {
            version: STORE_VERSION,
            next_id: 1,
            items: vec![Favorite {
                id: 1,
                path: "/tmp/other".to_string(),
                alias: Some("alpha".to_string()),
                tags: vec!["x".to_string()],
                uses: 0,
                last_used: None,
            }],
        };
        assert!(merge_store(&mut existing, imported).is_err());
    }

    #[test]
    fn load_store_seeds_on_missing() {
        let dir = tempdir().expect("tempdir");
        let config = dir.path().join("fav.json");
        let store = load_store(&config).expect("load");
        assert_eq!(store.items.len(), 1);
        assert_eq!(store.items[0].id, 1);
        assert_eq!(store.items[0].path, config.display().to_string());
    }

    #[test]
    fn save_and_load_store_roundtrip() {
        let dir = tempdir().expect("tempdir");
        let config = dir.path().join("nested/fav.json");
        let store = sample_store();
        save_store(&config, &store).expect("save");
        let loaded = load_store(&config).expect("load");
        assert_eq!(loaded.items.len(), store.items.len());
        assert_eq!(loaded.items[0].path, store.items[0].path);
    }

    #[test]
    fn format_path_variants() {
        let cwd = env::current_dir().expect("cwd");
        let cwd_str = cwd.to_string_lossy().to_string();
        let absolute = format_path(&cwd_str, PathFormat::Absolute).expect("absolute");
        assert_eq!(absolute, cwd_str);
        let relative = format_path(&cwd_str, PathFormat::Relative).expect("relative");
        assert_eq!(relative, ".");

        let nested = cwd.join("fav-subdir");
        let nested_str = nested.to_string_lossy().to_string();
        let nested_rel = format_path(&nested_str, PathFormat::Relative).expect("relative nested");
        assert_eq!(nested_rel, "fav-subdir");

        let outside = tempdir().expect("tempdir");
        let outside_str = outside.path().to_string_lossy().to_string();
        let outside_rel =
            format_path(&outside_str, PathFormat::Relative).expect("relative outside");
        assert_eq!(outside_rel, outside_str);

        if let Some(user_dirs) = UserDirs::new() {
            let home = user_dirs.home_dir().join("fav-test");
            let output =
                format_path(home.to_str().expect("home"), PathFormat::Tilde).expect("tilde");
            assert!(output.starts_with("~"));
        }

        let tmp_path = PathBuf::from("/tmp/fav-outside");
        let outside_tilde =
            format_path(tmp_path.to_str().expect("tmp"), PathFormat::Tilde).expect("tilde");
        assert_eq!(outside_tilde, tmp_path.display().to_string());
    }

    #[test]
    fn normalize_path_lenient_existing_and_missing() {
        let dir = tempdir().expect("tempdir");
        let file = dir.path().join("file.txt");
        fs::write(&file, "ok").expect("write");
        let existing = normalize_path_lenient(&file).expect("lenient");
        assert_eq!(existing, file.canonicalize().expect("canonicalize"));

        let missing = dir.path().join("missing.txt");
        let normalized = normalize_path_lenient(&missing).expect("missing");
        assert!(normalized.is_absolute());
        assert!(!normalized.exists());
    }

    #[test]
    fn expand_and_absolute_handles_absolute_and_relative() {
        let cwd = env::current_dir().expect("cwd");
        let absolute = expand_and_absolute(&cwd).expect("absolute");
        assert_eq!(absolute, cwd);

        let relative = PathBuf::from("relative-file");
        let expanded = expand_and_absolute(&relative).expect("relative");
        assert!(expanded.is_absolute());
        assert_eq!(expanded, cwd.join("relative-file"));
    }

    #[test]
    fn normalize_path_errors_on_missing() {
        let dir = tempdir().expect("tempdir");
        let missing = dir.path().join("missing.txt");
        assert!(normalize_path(&missing).is_err());
    }

    #[test]
    fn resolve_target_index_by_id_alias_and_path() {
        let dir = tempdir().expect("tempdir");
        let file = dir.path().join("alpha.txt");
        fs::write(&file, "ok").expect("write");
        let canonical = file.canonicalize().expect("canonicalize");
        let store = Store {
            version: STORE_VERSION,
            next_id: 2,
            items: vec![Favorite {
                id: 1,
                path: canonical.to_string_lossy().to_string(),
                alias: Some("alpha".to_string()),
                tags: Vec::new(),
                uses: 0,
                last_used: None,
            }],
        };
        assert_eq!(resolve_target_index(&store, "1").expect("id"), 0);
        assert_eq!(resolve_target_index(&store, "alpha").expect("alias"), 0);
        assert_eq!(
            resolve_target_index(&store, canonical.to_str().expect("path")).expect("path"),
            0
        );
    }

    #[test]
    fn validate_alias_rejects_invalid() {
        assert!(validate_alias("").is_err());
        assert!(validate_alias("list").is_err());
        assert!(validate_alias("123").is_err());
        assert!(validate_alias("okay").is_ok());
    }

    #[test]
    fn ensure_unique_alias_rejects_duplicates() {
        let store = sample_store();
        assert!(ensure_unique_alias(&store, "alpha").is_err());
        assert!(ensure_unique_alias(&store, "new").is_ok());
    }

    #[test]
    fn sort_by_path_tag_and_uses() {
        let store = sample_store();

        let mut by_path: Vec<&Favorite> = store.items.iter().collect();
        sort_items(&mut by_path, SortBy::Path, false);
        let path_ids: Vec<u64> = by_path.iter().map(|item| item.id).collect();
        assert_eq!(path_ids, vec![3, 1, 2]);

        let mut by_tag: Vec<&Favorite> = store.items.iter().collect();
        sort_items(&mut by_tag, SortBy::Tag, false);
        assert_eq!(by_tag.first().unwrap().id, 2);

        let mut by_uses: Vec<&Favorite> = store.items.iter().collect();
        sort_items(&mut by_uses, SortBy::Uses, false);
        let uses_ids: Vec<u64> = by_uses.iter().map(|item| item.id).collect();
        assert_eq!(uses_ids, vec![2, 1, 3]);
    }

    #[test]
    fn try_handle_alias_or_get_parsing() {
        let dir = tempdir().expect("tempdir");
        let config = dir.path().join("fav.json");
        let store = Store {
            version: STORE_VERSION,
            next_id: 2,
            items: vec![Favorite {
                id: 1,
                path: "/tmp/fav-alpha".to_string(),
                alias: Some("alpha".to_string()),
                tags: Vec::new(),
                uses: 0,
                last_used: None,
            }],
        };
        save_store(&config, &store).expect("save");

        let args = vec!["fav".to_string()];
        assert!(try_handle_alias_or_get(&args).expect("empty").is_none());

        let args = vec!["fav".to_string(), "--help".to_string()];
        assert!(try_handle_alias_or_get(&args).expect("help").is_none());

        let args = vec![
            "fav".to_string(),
            "--config".to_string(),
            config.display().to_string(),
            "alpha".to_string(),
        ];
        let output = try_handle_alias_or_get(&args).expect("alias");
        assert_eq!(output, Some("/tmp/fav-alpha".to_string()));

        let args = vec![
            "fav".to_string(),
            format!("--config={}", config.display()),
            "1".to_string(),
        ];
        let output = try_handle_alias_or_get(&args).expect("id");
        assert_eq!(output, Some("/tmp/fav-alpha".to_string()));

        let args = vec!["fav".to_string(), "alpha".to_string(), "extra".to_string()];
        assert!(try_handle_alias_or_get(&args).expect("extra").is_none());
    }
}

fn normalize_path(input: &Path) -> Result<PathBuf> {
    let absolute = expand_and_absolute(input)?;
    let canonical = absolute
        .canonicalize()
        .with_context(|| format!("Path does not exist: {}", absolute.display()))?;
    Ok(canonical)
}

fn normalize_tags(tags: Vec<String>) -> Vec<String> {
    tags.into_iter()
        .flat_map(|tag| {
            tag.split(',')
                .map(|t| t.trim().to_string())
                .collect::<Vec<_>>()
        })
        .filter(|tag| !tag.is_empty())
        .collect()
}

fn resolve_target_index(store: &Store, target: &str) -> Result<usize> {
    if let Ok(id) = target.parse::<u64>() {
        return store
            .items
            .iter()
            .position(|item| item.id == id)
            .context("No favorite with that id");
    }
    if let Some(idx) = store
        .items
        .iter()
        .position(|item| item.alias.as_deref() == Some(target))
    {
        return Ok(idx);
    }
    let normalized = normalize_path_lenient(Path::new(target))?;
    let normalized = normalized.to_string_lossy();
    store
        .items
        .iter()
        .position(|item| item.path == normalized || item.path == target)
        .context("No favorite with that path")
}

fn validate_alias(alias: &str) -> Result<()> {
    if alias.trim().is_empty() {
        bail!("Alias cannot be empty");
    }
    if is_reserved_word(alias) {
        bail!("Alias cannot be a reserved command");
    }
    if alias.chars().all(|c| c.is_ascii_digit()) {
        bail!("Alias cannot be purely numeric");
    }
    Ok(())
}

fn ensure_unique_alias(store: &Store, alias: &str) -> Result<()> {
    if store
        .items
        .iter()
        .any(|item| item.alias.as_deref() == Some(alias))
    {
        bail!("Alias already in use");
    }
    Ok(())
}

fn expand_and_absolute(input: &Path) -> Result<PathBuf> {
    let raw = input.to_string_lossy();
    let expanded = shellexpand::full(raw.as_ref())
        .context("expand path")
        .map(|s| s.into_owned())?;
    let path = PathBuf::from(expanded);
    let absolute = if path.is_absolute() {
        path
    } else {
        env::current_dir()
            .context("get current directory")?
            .join(path)
    };
    Ok(absolute)
}

fn normalize_path_lenient(input: &Path) -> Result<PathBuf> {
    let absolute = expand_and_absolute(input)?;
    if absolute.exists() {
        Ok(absolute.canonicalize().context("canonicalize path")?)
    } else {
        Ok(absolute)
    }
}

fn format_path(path: &str, format: PathFormat) -> Result<String> {
    let path = Path::new(path);
    match format {
        PathFormat::Absolute => Ok(path.display().to_string()),
        PathFormat::Tilde => {
            if let Some(user_dirs) = UserDirs::new() {
                let home = user_dirs.home_dir();
                if let Ok(stripped) = path.strip_prefix(home) {
                    return Ok(format!("~{}", Path::new("/").join(stripped).display()));
                }
            }
            Ok(path.display().to_string())
        }
        PathFormat::Relative => {
            let cwd = env::current_dir().context("get current directory")?;
            if let Ok(stripped) = path.strip_prefix(&cwd) {
                if stripped.as_os_str().is_empty() {
                    return Ok(".".to_string());
                }
                return Ok(stripped.display().to_string());
            }
            Ok(path.display().to_string())
        }
    }
}
