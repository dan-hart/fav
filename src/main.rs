use std::collections::{HashMap, HashSet};
use std::env;
use std::fs;
use std::io::{self, IsTerminal, Read, Write};
use std::path::{Path, PathBuf};
use std::process::Command as ProcessCommand;
#[cfg(not(feature = "coverage"))]
use std::process::Stdio;
#[cfg(not(feature = "coverage"))]
use std::time::Duration;
use std::time::{SystemTime, UNIX_EPOCH};

use anyhow::{Context, Result, bail};
use clap::{Args, CommandFactory, Parser, Subcommand, ValueEnum};
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

mod automation;

// Route human output through the response collector when --json is enabled.
macro_rules! println {
    ($($arg:tt)*) => { automation::line(format!($($arg)*)) };
}
macro_rules! eprintln {
    ($($arg:tt)*) => { automation::warning(format!($($arg)*)) };
}

const STORE_VERSION: u32 = 2;

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
    /// Emit a versioned JSON response (including structured errors)
    #[arg(long, global = true)]
    json: bool,
    /// Refuse interactive selection and terminal stdin reads
    #[arg(long, global = true)]
    non_interactive: bool,
    /// Preview changes without saving or executing commands
    #[arg(long, global = true)]
    dry_run: bool,
    /// Path to the favorites config (defaults to ~/.fav.config)
    #[arg(long, global = true)]
    config: Option<PathBuf>,
    #[command(subcommand)]
    command: Option<Command>,
}

#[derive(Subcommand)]
enum Command {
    #[command(
        about = "Describe commands and response contracts as JSON",
        after_help = "Example: fav schema"
    )]
    Schema,
    #[command(
        about = "Ensure a path is favorited; update only supplied metadata",
        after_help = "Example: fav ensure ./project --alias project --tag active --json"
    )]
    Ensure(AddArgs),
    #[command(
        about = "Resolve a favorite without updating usage",
        after_help = "Example: fav resolve project --json"
    )]
    Resolve(GetArgs),
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
Examples:\n  fav meta 1 --alias dotfiles\n  fav meta 1 --clear-alias\n  fav meta 1 --tag work,urgent\n  fav meta 1 --set-tags --tag work,urgent\n  fav meta 1 --rm-tag urgent\n  fav meta 1 --rename-tag old=new\n"
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
    #[command(
        about = "Generate shell helpers",
        after_help = "Example: eval \"$(fav shell init zsh)\""
    )]
    Shell(ShellArgs),
    #[command(
        about = "Open a favorite with the system opener",
        after_help = "Example: fav open project --dry-run"
    )]
    Open(OpenArgs),
    #[command(
        about = "Manage reusable command presets",
        after_help = "Example: fav preset add search -- rg TODO {}"
    )]
    Preset(PresetArgs),
    #[command(
        about = "Inspect duplicates or repair paths",
        after_help = "Example: fav doctor repair --from ~/old --to ~/new --dry-run"
    )]
    Doctor(DoctorArgs),
    #[command(
        about = "Import favorites from shell history or path files",
        after_help = "Example: fav import paths --file paths.json --dry-run"
    )]
    Import(ImportArgs),
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
    /// Optional note or description
    #[arg(long)]
    note: Option<String>,
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
    /// Structured query (for example: tag:work note:project -tag:archive)
    #[arg(long)]
    query: Option<String>,
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
    /// Use smarter ranking based on exact matches, usage, and recency
    #[arg(long)]
    smart: bool,
    /// Include notes in human-readable output
    #[arg(long)]
    show_notes: bool,
}

#[derive(Args)]
struct GetArgs {
    /// Resolve without modifying usage counters or timestamps
    #[arg(long)]
    no_touch: bool,
    /// Target id or alias
    target: String,
    /// How to render paths
    #[arg(long, value_enum, default_value = "absolute")]
    path_format: PathFormat,
}

#[derive(Args)]
struct MetaArgs {
    /// Target id/alias/path
    target: Option<String>,
    /// Structured query to target multiple favorites
    #[arg(long)]
    query: Option<String>,
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
    /// Set or replace note text
    #[arg(long)]
    note: Option<String>,
    /// Clear note text
    #[arg(long)]
    clear_note: bool,
    /// Confirm query-based updates
    #[arg(long)]
    yes: bool,
}

#[derive(Args)]
struct PickArgs {
    /// Filter by tag (repeatable; all tags must match)
    #[arg(long)]
    tag: Vec<String>,
    /// Search query (matches alias/path/tags)
    #[arg(long)]
    search: Option<String>,
    /// Structured query (for example: tag:work note:project)
    #[arg(long)]
    query: Option<String>,
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
    /// Use smarter ranking based on exact matches, usage, and recency
    #[arg(long)]
    smart: bool,
}

#[derive(Args)]
struct TuiArgs {
    /// Filter by tag (repeatable; all tags must match)
    #[arg(long)]
    tag: Vec<String>,
    /// Initial search query (matches alias/path/tags)
    #[arg(long)]
    search: Option<String>,
    /// Structured query (for example: tag:work note:project)
    #[arg(long)]
    query: Option<String>,
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
    /// Use smarter ranking based on exact matches, usage, and recency
    #[arg(long)]
    smart: bool,
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
    target: Option<String>,
    /// Structured query to target multiple favorites
    #[arg(long)]
    query: Option<String>,
    /// Confirm query-based removals
    #[arg(long)]
    yes: bool,
}

#[derive(Args)]
struct ShellArgs {
    #[command(subcommand)]
    command: ShellCommand,
}

#[derive(Subcommand)]
enum ShellCommand {
    #[command(
        about = "Print shell functions for the selected shell",
        after_help = "Examples: fav shell init bash; fav shell init fish"
    )]
    Init(ShellInitArgs),
}

#[derive(Args)]
struct ShellInitArgs {
    /// Shell syntax to generate (requires the corresponding shell)
    shell: ShellKind,
}

#[derive(Args)]
struct OpenArgs {
    /// Favorite id, alias, or stored path
    target: String,
}

#[derive(Args)]
struct PresetArgs {
    #[command(subcommand)]
    command: PresetCommand,
}

#[derive(Subcommand)]
enum PresetCommand {
    #[command(
        about = "Save a command template",
        after_help = "Example: fav preset add search -- rg TODO {}"
    )]
    Add(PresetAddArgs),
    #[command(
        about = "List templates in name order",
        after_help = "Example: fav preset list --json"
    )]
    List,
    #[command(
        about = "Execute a template using a favorite",
        after_help = "Example: fav preset run search project --dry-run"
    )]
    Run(PresetRunArgs),
    #[command(
        about = "Remove a saved template",
        after_help = "Example: fav preset rm search --dry-run"
    )]
    Rm(PresetRemoveArgs),
}

#[derive(Args)]
struct PresetAddArgs {
    /// Unique template name
    name: String,
    /// Optional description
    #[arg(long)]
    note: Option<String>,
    /// Command tokens after --; {} is replaced with the favorite path
    #[arg(required = true, trailing_var_arg = true, allow_hyphen_values = true)]
    command: Vec<String>,
}

#[derive(Args)]
struct PresetRunArgs {
    /// Saved template name
    name: String,
    /// Favorite id, alias, or path
    target: String,
    /// Path representation passed to the child process
    #[arg(long, value_enum, default_value = "absolute")]
    path_format: PathFormat,
}

#[derive(Args)]
struct PresetRemoveArgs {
    /// Saved template name
    name: String,
}

#[derive(Args)]
struct DoctorArgs {
    #[command(subcommand)]
    command: DoctorCommand,
}

#[derive(Subcommand)]
enum DoctorCommand {
    #[command(
        about = "Report duplicate paths and normalized alias collisions",
        after_help = "Example: fav doctor duplicates --json"
    )]
    Duplicates,
    #[command(
        about = "Repair missing paths by replacing a root prefix",
        after_help = "Example: fav doctor repair --from ~/old --to ~/new --dry-run"
    )]
    Repair(DoctorRepairArgs),
}

#[derive(Args)]
struct DoctorRepairArgs {
    /// Old path root to replace
    #[arg(long)]
    from: PathBuf,
    /// New root; replacements must exist
    #[arg(long)]
    to: PathBuf,
}

#[derive(Args)]
struct ImportArgs {
    #[command(subcommand)]
    command: ImportCommand,
}

#[derive(Subcommand)]
enum ImportCommand {
    #[command(
        about = "Import existing paths found in shell history",
        after_help = "Example: fav import history --shell zsh --limit 50 --dry-run"
    )]
    History(ImportHistoryArgs),
    #[command(
        about = "Import newline, CSV, or JSON paths",
        after_help = "Example: fav import paths --file paths.json --tag imported --dry-run"
    )]
    Paths(ImportPathsArgs),
}

#[derive(Args)]
struct ImportHistoryArgs {
    /// History syntax; auto examines filenames and contents
    #[arg(long, value_enum, default_value = "auto")]
    shell: HistoryShell,
    /// History file (otherwise search the user's home directory)
    #[arg(long)]
    file: Option<PathBuf>,
    /// Maximum number of recent commands to inspect
    #[arg(long)]
    limit: Option<usize>,
    /// Tags attached to imported favorites
    #[arg(long, value_delimiter = ',')]
    tag: Vec<String>,
    /// Note attached to imported favorites
    #[arg(long)]
    note: Option<String>,
}

#[derive(Args)]
struct ImportPathsArgs {
    /// Input file (otherwise read stdin)
    #[arg(long)]
    file: Option<PathBuf>,
    /// Tags attached to imported favorites
    #[arg(long, value_delimiter = ',')]
    tag: Vec<String>,
    /// Note attached to imported favorites
    #[arg(long)]
    note: Option<String>,
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

#[derive(Clone, Copy, Debug, ValueEnum)]
enum ShellKind {
    Zsh,
    Bash,
    Fish,
}

#[derive(Clone, Copy, Debug, ValueEnum, PartialEq, Eq)]
enum HistoryShell {
    Auto,
    Zsh,
    Bash,
    Fish,
}

#[derive(Debug, Serialize, Deserialize, Default)]
struct Store {
    version: u32,
    next_id: u64,
    #[serde(default)]
    items: Vec<Favorite>,
    #[serde(default)]
    presets: Vec<Preset>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
struct Preset {
    name: String,
    command: Vec<String>,
    #[serde(default)]
    note: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
struct Favorite {
    id: u64,
    path: String,
    alias: Option<String>,
    tags: Vec<String>,
    #[serde(default)]
    note: Option<String>,
    #[serde(default)]
    uses: u64,
    #[serde(default)]
    last_used: Option<i64>,
}

fn main() {
    let mut args: Vec<String> = env::args().collect();
    let flags: Vec<&str> = args
        .iter()
        .skip(1)
        .take_while(|arg| *arg != "--")
        .map(String::as_str)
        .collect();
    automation::init(flags.contains(&"--json"), flags.contains(&"--dry-run"));
    // Keep the speed-dial shortcut, including global flags, on the same execution path.
    let mut skip = false;
    for index in 1..args.len() {
        if args[index] == "--" {
            break;
        }
        if skip {
            skip = false;
            continue;
        }
        if args[index] == "--config" {
            skip = true;
            continue;
        }
        if args[index].starts_with('-') {
            continue;
        }
        if !is_reserved_word(&args[index]) {
            args.insert(index, "get".to_string());
        }
        break;
    }
    let cli = match Cli::try_parse_from(&args) {
        Ok(cli) => cli,
        Err(error) => {
            if matches!(
                error.kind(),
                clap::error::ErrorKind::DisplayHelp | clap::error::ErrorKind::DisplayVersion
            ) {
                print!("{error}");
                return;
            }
            let failure = automation::Failure {
                code: "invalid_arguments",
                exit: 2,
                message: error.to_string(),
            };
            std::process::exit(automation::error(&failure.into()));
        }
    };
    let command_name = match &cli.command {
        None | Some(Command::List(_)) => "list",
        Some(Command::Add(_)) => "add",
        Some(Command::Ensure(_)) => "ensure",
        Some(Command::Get(_)) => "get",
        Some(Command::Resolve(_)) => "resolve",
        Some(Command::Schema) => "schema",
        Some(Command::Meta(_)) => "meta",
        Some(Command::Rm(_)) => "rm",
        Some(Command::Io(_)) => "io",
        Some(Command::Health(_)) => "health",
        Some(Command::Pick(_)) => "pick",
        Some(Command::Tui(_)) => "tui",
        Some(Command::With(_)) => "with",
        Some(Command::Shell(_)) => "shell",
        Some(Command::Open(_)) => "open",
        Some(Command::Preset(_)) => "preset",
        Some(Command::Doctor(_)) => "doctor",
        Some(Command::Import(_)) => "import",
    };
    if let Err(error) = run_cli(cli) {
        std::process::exit(automation::error(&error));
    }
    automation::finish(command_name);
}

fn command_schema(command: &clap::Command) -> serde_json::Value {
    serde_json::json!({
        "name": command.get_name(),
        "aliases": command.get_all_aliases().collect::<Vec<_>>(),
        "description": command.get_about().map(ToString::to_string),
        "help": command.clone().render_long_help().to_string(),
        "arguments": command.get_arguments().map(|arg| serde_json::json!({
            "name": arg.get_id().as_str(), "long": arg.get_long(), "short": arg.get_short(),
            "required": arg.is_required_set(), "global": arg.is_global_set(),
            "help": arg.get_help().map(ToString::to_string),
            "action": format!("{:?}", arg.get_action()),
            "values": arg.get_value_parser().possible_values().map(|values| values.map(|value| value.get_name().to_string()).collect::<Vec<_>>()),
            "defaults": arg.get_default_values().iter().map(|v| v.to_string_lossy()).collect::<Vec<_>>()
        })).collect::<Vec<_>>(),
        "commands": command.get_subcommands().map(command_schema).collect::<Vec<_>>()
    })
}

fn run_cli(cli: Cli) -> Result<()> {
    if matches!(cli.command, Some(Command::Schema)) {
        let mut command = Cli::command();
        command.build();
        let schema = serde_json::json!({"schema_version": 1, "version": env!("CARGO_PKG_VERSION"),
            "cli": command_schema(&command), "response": {"schema_version": 1, "ok": "boolean", "command": "canonical top-level command", "dry_run": "boolean", "data": "command-specific JSON", "items": "changed favorites", "changes": "id/before/after objects", "presets": "resulting presets", "changed_ids": "sorted ids", "output": "text lines", "warnings": "array"},
            "exit_codes": {"0": "success", "2": "invalid input", "3": "not found", "4": "conflict", "5": "busy", "6": "storage", "7": "execution failed"}});
        if cli.json {
            automation::data(schema);
        } else {
            std::println!("{schema}");
        }
        return Ok(());
    }
    let store_path = resolve_config_path(cli.config.as_deref())?;
    let stdin_required = matches!(
        &cli.command,
        Some(Command::Io(IoArgs {
            import: true,
            file: None,
            ..
        })) | Some(Command::Import(ImportArgs {
            command: ImportCommand::Paths(ImportPathsArgs { file: None, .. })
        }))
    );
    if (cli.non_interactive || cli.json) && stdin_required && io::stdin().is_terminal() {
        bail!("Terminal stdin is disabled; supply --file or pipe input.");
    }
    automation::lock(&store_path)?;
    let mut store = load_store(&store_path)?;
    automation::before(serde_json::to_value(&store)?);
    let dry_run = cli.dry_run;
    if (cli.non_interactive || cli.json || cli.dry_run)
        && matches!(cli.command, Some(Command::Pick(_) | Command::Tui(_)))
    {
        bail!("Interactive selection is disabled. Use fav list --json and fav resolve instead.");
    }

    match cli.command.unwrap_or(Command::List(ListArgs {
        tag: Vec::new(),
        search: None,
        query: None,
        format: OutputFormat::Table,
        path_format: PathFormat::Relative,
        sort: SortBy::Id,
        reverse: false,
        smart: false,
        show_notes: false,
    })) {
        Command::Schema => unreachable!(),
        Command::Ensure(args) => {
            let path = normalize_path(&args.path.unwrap_or(env::current_dir()?))?
                .to_string_lossy()
                .to_string();
            if let Some(alias) = args.alias.as_deref() {
                validate_alias(alias)?;
            }
            let index = store.items.iter().position(|item| item.path == path);
            if let Some(alias) = args.alias.as_deref() {
                if let Some(index) = index {
                    ensure_unique_alias_except(&store, alias, index)?;
                } else {
                    ensure_unique_alias(&store, alias)?;
                }
            }
            let index = index.unwrap_or_else(|| {
                let id = store.next_id.max(1);
                store.next_id = id + 1;
                store.items.push(Favorite {
                    id,
                    path,
                    alias: None,
                    tags: Vec::new(),
                    note: None,
                    uses: 0,
                    last_used: None,
                });
                store.items.len() - 1
            });
            let item = &mut store.items[index];
            if let Some(alias) = args.alias {
                item.alias = Some(alias);
            }
            if let Some(note) = args.note {
                item.note = Some(note);
            }
            item.tags.extend(normalize_tags(args.tag));
            item.tags.sort();
            item.tags.dedup();
            automation::data(serde_json::to_value(&*item)?);
            println!("{}", item.id);
            save_store(&store_path, &store)?;
        }
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
                note: args.note,
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
                query: args.query.as_deref().map(parse_query).transpose()?,
            };
            let opts = ListOptions {
                format: args.format,
                path_format: args.path_format,
                sort: args.sort,
                reverse: args.reverse,
                smart: args.smart,
                show_notes: args.show_notes,
            };
            let count = if cli.json {
                let mut items = filtered_items(&store, &filters);
                sort_items(
                    &mut items,
                    opts.sort,
                    opts.reverse,
                    &filters,
                    opts.smart,
                    None,
                );
                automation::data(serde_json::to_value(&items)?);
                items.len()
            } else {
                list_items(&store, &filters, &opts)?
            };
            if count == 0 {
                eprintln!("No favorites matched. Try: fav add, fav list --search <term>");
            }
        }
        Command::Resolve(args) => {
            let idx = resolve_target_index(&store, &args.target)?;
            let path = format_path(&store.items[idx].path, args.path_format)?;
            automation::data(serde_json::json!({"id": store.items[idx].id, "path": path}));
            println!("{path}");
        }
        Command::Get(args) => {
            let idx = resolve_target_index(&store, &args.target)?;
            if !args.no_touch {
                mark_used(&mut store.items[idx]);
            }
            let output = format_path(store.items[idx].path.as_str(), args.path_format)?;
            if !args.no_touch {
                save_store(&store_path, &store)?;
            }
            automation::data(serde_json::json!({"id": store.items[idx].id, "path": output}));
            println!("{output}");
        }
        Command::Meta(args) => {
            let indices =
                resolve_target_indices(&store, args.target.as_deref(), args.query.as_deref())?;
            let has_alias = args.alias.is_some();
            let has_clear_alias = args.clear_alias;
            let has_add_tags = !args.tag.is_empty();
            let has_rm_tags = !args.rm_tag.is_empty();
            let has_rename_tag = args.rename_tag.is_some();
            let has_note = args.note.is_some();
            let has_clear_note = args.clear_note;
            let tag_actions = has_add_tags as u8 + has_rm_tags as u8 + has_rename_tag as u8;
            if has_alias && has_clear_alias {
                bail!("Choose only one of: --alias or --clear-alias");
            }
            if has_note && has_clear_note {
                bail!("Choose only one of: --note or --clear-note");
            }
            if tag_actions > 1 {
                bail!("Choose only one of: --tag, --rm-tag, or --rename-tag");
            }
            if args.set_tags && !has_add_tags {
                bail!("--set-tags can only be used with --tag");
            }
            if args.query.is_some() && !args.yes && !dry_run {
                bail!("Use --yes with --query to update matching favorites.");
            }
            if !has_alias && !has_clear_alias && tag_actions == 0 && !has_note && !has_clear_note {
                bail!("Provide metadata to update (alias, tags, or note). See `fav meta --help`.");
            }
            if (has_alias || has_clear_alias) && indices.len() != 1 {
                bail!("Alias updates require exactly one matched favorite.");
            }

            let mut alias_updated = false;
            let mut tags_updated = false;
            let mut note_updated = false;

            if has_clear_alias {
                let idx = indices[0];
                store.items[idx].alias = None;
                alias_updated = true;
            } else if let Some(alias) = args.alias.as_deref() {
                let idx = indices[0];
                validate_alias(alias)?;
                if store.items[idx].alias.as_deref() != Some(alias) {
                    ensure_unique_alias_except(&store, alias, idx)?;
                    store.items[idx].alias = Some(alias.to_string());
                    alias_updated = true;
                }
            }

            if let Some(rename) = args.rename_tag.as_deref() {
                let (old, new) = parse_rename(rename)?;
                for idx in &indices {
                    let tags = &mut store.items[*idx].tags;
                    for tag in tags.iter_mut() {
                        if tag.eq_ignore_ascii_case(old) {
                            *tag = new.to_string();
                        }
                    }
                    tags.sort();
                    tags.dedup();
                }
                tags_updated = true;
            } else if has_rm_tags {
                let remove = normalize_tags(args.rm_tag);
                for idx in &indices {
                    store.items[*idx]
                        .tags
                        .retain(|tag| !remove.iter().any(|rm| rm.eq_ignore_ascii_case(tag)));
                }
                tags_updated = true;
            } else if has_add_tags {
                let tags = normalize_tags(args.tag);
                for idx in &indices {
                    if args.set_tags {
                        store.items[*idx].tags = tags.clone();
                    } else {
                        store.items[*idx].tags.extend(tags.clone());
                        store.items[*idx].tags.sort();
                        store.items[*idx].tags.dedup();
                    }
                }
                tags_updated = true;
            }

            if has_clear_note {
                for idx in &indices {
                    store.items[*idx].note = None;
                }
                note_updated = true;
            } else if let Some(note) = args.note.as_deref() {
                for idx in &indices {
                    store.items[*idx].note = Some(note.to_string());
                }
                note_updated = true;
            }

            save_store(&store_path, &store)?;
            if indices.len() > 1 {
                println!("Updated {} favorites.", indices.len());
            } else {
                let item = &store.items[indices[0]];
                let mut updated_fields = Vec::new();
                if alias_updated {
                    updated_fields.push("alias");
                }
                if tags_updated {
                    updated_fields.push("tags");
                }
                if note_updated {
                    updated_fields.push("note");
                }
                if updated_fields.len() == 1 && updated_fields[0] == "alias" {
                    println!(
                        "Updated alias for {}: {}",
                        item.id,
                        item.alias.as_deref().unwrap_or("-")
                    );
                } else if updated_fields.len() == 1 && updated_fields[0] == "tags" {
                    let tags = if item.tags.is_empty() {
                        "-".to_string()
                    } else {
                        item.tags.join(",")
                    };
                    println!("Updated tags for {}: {tags}", item.id);
                } else if updated_fields.len() == 1 && updated_fields[0] == "note" {
                    println!(
                        "Updated note for {}: {}",
                        item.id,
                        item.note.as_deref().unwrap_or("-")
                    );
                } else {
                    println!("Updated {} for {}.", updated_fields.join(", "), item.id);
                }
            }
        }
        Command::Pick(args) => {
            let filters = Filters {
                search: args.search.clone(),
                tags: args.tag.clone(),
                query: args.query.as_deref().map(parse_query).transpose()?,
            };
            let opts = ListOptions {
                format: OutputFormat::Plain,
                path_format: args.display_path_format,
                sort: args.sort,
                reverse: args.reverse,
                smart: args.smart,
                show_notes: false,
            };
            automation::unlock();
            let selection = pick_item(&store, &filters, &opts)?;
            if let Some(id) = selection {
                automation::lock(&store_path)?;
                store = load_store(&store_path)?;
                automation::before(serde_json::to_value(&store)?);
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
            automation::unlock();
            let selection = run_tui(&store, &args)?;
            if let Some(id) = selection {
                automation::lock(&store_path)?;
                store = load_store(&store_path)?;
                automation::before(serde_json::to_value(&store)?);
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
                bail!("Choose exactly one of --export or --import. See `fav io --help`.");
            }
            if args.merge && !args.import {
                bail!("--merge can only be used with --import");
            }
            if args.export {
                let has_file = args.file.is_some();
                if cli.json {
                    automation::data(serde_json::to_value(&store)?);
                }
                if !cli.json || args.file.is_some() {
                    export_store(&store, args.file)?;
                }
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
            automation::data(serde_json::to_value(
                store
                    .items
                    .iter()
                    .filter(|item| !Path::new(&item.path).exists())
                    .collect::<Vec<_>>(),
            )?);
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
            if !dry_run {
                mark_used(&mut store.items[idx]);
            }
            let path = format_path(store.items[idx].path.as_str(), args.path_format)?;
            save_store(&store_path, &store)?;
            run_with_command(&args.command, &path, dry_run)?;
        }
        Command::Rm(args) => {
            let indices =
                resolve_target_indices(&store, args.target.as_deref(), args.query.as_deref())?;
            if args.query.is_some() && !args.yes && !dry_run {
                bail!("Use --yes with --query to remove matching favorites.");
            }
            let removed = indices.len();
            for idx in indices.into_iter().rev() {
                store.items.remove(idx);
            }
            save_store(&store_path, &store)?;
            if removed == 1 {
                println!("Removed favorite.");
            } else {
                println!("Removed {removed} favorites.");
            }
        }
        Command::Shell(args) => match args.command {
            ShellCommand::Init(args) => {
                let script = render_shell_init(args.shell);
                automation::data(serde_json::json!({"script": script}));
                println!("{script}");
            }
        },
        Command::Open(args) => {
            let idx = resolve_target_index(&store, &args.target)?;
            let path = store.items[idx].path.clone();
            if dry_run {
                automation::data(serde_json::json!({"program": opener_program()?, "args": [path]}));
                println!("{}", render_open_command(&path)?);
            } else {
                mark_used(&mut store.items[idx]);
                save_store(&store_path, &store)?;
                automation::unlock();
                run_open_command(&path)?;
            }
        }
        Command::Preset(args) => match args.command {
            PresetCommand::Add(args) => {
                validate_preset_name(&args.name)?;
                ensure_unique_preset(&store, &args.name)?;
                if args.command.is_empty() {
                    bail!(
                        "Provide a command after -- (example: fav preset add show -- printf %s {{}})"
                    );
                }
                store.presets.push(Preset {
                    name: args.name,
                    command: args.command,
                    note: args.note,
                });
                sync_store(&mut store);
                save_store(&store_path, &store)?;
                println!("Added preset.");
            }
            PresetCommand::List => {
                automation::data(serde_json::to_value(&store.presets)?);
                for preset in &store.presets {
                    println!(
                        "{}\t{}\t{}",
                        preset.name,
                        preset.note.as_deref().unwrap_or("-"),
                        render_command_line(&preset.command[0], &preset.command[1..])
                    );
                }
            }
            PresetCommand::Run(args) => {
                let preset = find_preset(&store, &args.name)?.clone();
                let idx = resolve_target_index(&store, &args.target)?;
                let path = format_path(store.items[idx].path.as_str(), args.path_format)?;
                if !dry_run {
                    mark_used(&mut store.items[idx]);
                    save_store(&store_path, &store)?;
                }
                run_with_command(&preset.command, &path, dry_run)?;
            }
            PresetCommand::Rm(args) => {
                let before = store.presets.len();
                store.presets.retain(|preset| preset.name != args.name);
                if store.presets.len() == before {
                    bail!("No preset named '{}'.", args.name);
                }
                save_store(&store_path, &store)?;
                println!("Removed preset.");
            }
        },
        Command::Doctor(args) => match args.command {
            DoctorCommand::Duplicates => {
                let duplicates = duplicate_index(&store);
                automation::data(serde_json::to_value(
                    store
                        .items
                        .iter()
                        .filter(|item| is_duplicate_item(item, &duplicates))
                        .collect::<Vec<_>>(),
                )?);
                report_duplicates(&store)?;
            }
            DoctorCommand::Repair(args) => {
                let repaired = repair_paths(&mut store, &args.from, &args.to, false)?;
                if !dry_run {
                    save_store(&store_path, &store)?;
                }
                eprintln!("Repaired {} favorites.", repaired);
            }
        },
        Command::Import(args) => match args.command {
            ImportCommand::History(args) => {
                let report = import_history(&mut store, &args)?;
                automation::data(
                    serde_json::json!({"added": report.added, "skipped": report.skipped}),
                );
                save_store(&store_path, &store)?;
                eprintln!(
                    "Imported {} favorites (skipped {}).",
                    report.added, report.skipped
                );
            }
            ImportCommand::Paths(args) => {
                let report = import_paths_file(&mut store, &args)?;
                automation::data(
                    serde_json::json!({"added": report.added, "skipped": report.skipped}),
                );
                save_store(&store_path, &store)?;
                eprintln!(
                    "Imported {} favorites (skipped {}).",
                    report.added, report.skipped
                );
            }
        },
    }

    automation::after(serde_json::to_value(&store)?);
    Ok(())
}

#[cfg(test)]
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
            | "ensure"
            | "resolve"
            | "schema"
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
            | "shell"
            | "open"
            | "preset"
            | "doctor"
            | "import"
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
                note: None,
                uses: 0,
                last_used: None,
            }],
            presets: Vec::new(),
        });
    }

    let data = fs::read_to_string(store_path).context("read favorites store")?;
    let mut store: Store = serde_json::from_str(&data).context("parse favorites store")?;
    validate_store(&store)?;
    sync_store(&mut store);
    Ok(store)
}

fn save_store(path: &Path, store: &Store) -> Result<()> {
    if automation::is_dry_run() {
        return Ok(());
    }
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).context("create data directory")?;
    }
    let data = serde_json::to_vec(store).context("serialize favorites store")?;
    let parent = path
        .parent()
        .context("config requires a parent directory")?;
    let mut temporary =
        tempfile::NamedTempFile::new_in(parent).context("create temporary store")?;
    temporary
        .write_all(&data)
        .context("write favorites store")?;
    temporary
        .as_file()
        .sync_all()
        .context("sync favorites store")?;
    temporary.persist(path).context("save favorites store")?;
    Ok(())
}

fn resolve_config_path(override_path: Option<&Path>) -> Result<PathBuf> {
    let path = if let Some(path) = override_path {
        expand_and_absolute(path)?
    } else if let Some(env_path) = env::var_os("FAV_CONFIG") {
        expand_and_absolute(Path::new(&env_path))?
    } else {
        UserDirs::new()
            .context("resolve home directory")?
            .home_dir()
            .join(".fav.config")
    };
    if path.exists() {
        return path.canonicalize().context("resolve favorites store");
    }
    if let (Some(parent), Some(name)) = (path.parent(), path.file_name())
        && let Ok(parent) = parent.canonicalize()
    {
        return Ok(parent.join(name));
    }
    Ok(path)
}

fn sync_store(store: &mut Store) {
    if store.version < STORE_VERSION {
        store.version = STORE_VERSION;
    }
    let max_id = store.items.iter().map(|item| item.id).max().unwrap_or(0);
    if store.next_id <= max_id {
        store.next_id = max_id + 1;
    }
    store.items.sort_by_key(|item| item.id);
    store
        .presets
        .sort_by(|left, right| left.name.cmp(&right.name));
}

fn validate_store(store: &Store) -> Result<()> {
    if store.presets.iter().any(|preset| preset.command.is_empty()) {
        bail!("Invalid favorites store: presets require a non-empty command");
    }
    Ok(())
}

struct Filters {
    search: Option<String>,
    tags: Vec<String>,
    query: Option<QuerySpec>,
}

struct ListOptions {
    format: OutputFormat,
    path_format: PathFormat,
    sort: SortBy,
    reverse: bool,
    smart: bool,
    show_notes: bool,
}

#[derive(Serialize)]
struct OutputItem {
    id: u64,
    alias: Option<String>,
    path: String,
    tags: Vec<String>,
    note: Option<String>,
    uses: u64,
    last_used: Option<i64>,
}

fn list_items(store: &Store, filters: &Filters, opts: &ListOptions) -> Result<usize> {
    let mut items = filtered_items(store, filters);
    sort_items(
        &mut items,
        opts.sort,
        opts.reverse,
        filters,
        opts.smart,
        None,
    );
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
                if opts.show_notes {
                    let note = item.note.as_deref().unwrap_or("-");
                    println!(
                        "{:>4}  {:<20}  {:<54}  {:<20}  {}",
                        item.id, alias, path, tags, note
                    );
                } else {
                    println!("{:>4}  {:<20}  {:<54}  {}", item.id, alias, path, tags);
                }
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
                if opts.show_notes {
                    println!(
                        "{}\t{}\t{}\t{}\t{}",
                        item.id,
                        alias,
                        path,
                        tags,
                        item.note.as_deref().unwrap_or("-")
                    );
                } else {
                    println!("{}\t{}\t{}\t{}", item.id, alias, path, tags);
                }
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
                    note: item.note.clone(),
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
    let duplicates = duplicate_index(store);
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
                && !matches_text_query(item, q)
            {
                return false;
            }
            if let Some(query) = filters.query.as_ref()
                && !matches_query_spec(item, query, &duplicates)
            {
                return false;
            }
            true
        })
        .collect()
}

fn matches_text_query(item: &Favorite, query: &str) -> bool {
    item.alias
        .as_ref()
        .is_some_and(|alias| alias.to_lowercase().contains(query))
        || item.path.to_lowercase().contains(query)
        || item
            .note
            .as_ref()
            .is_some_and(|note| note.to_lowercase().contains(query))
        || item
            .tags
            .iter()
            .any(|tag| tag.to_lowercase().contains(query))
}

fn sort_items(
    items: &mut Vec<&Favorite>,
    sort: SortBy,
    reverse: bool,
    filters: &Filters,
    smart: bool,
    query_hint: Option<&str>,
) {
    if smart {
        items.sort_by(|left, right| {
            smart_score(right, filters, query_hint)
                .cmp(&smart_score(left, filters, query_hint))
                .then_with(|| left.id.cmp(&right.id))
        });
        if reverse {
            items.reverse();
        }
        return;
    }

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

#[derive(Clone, Debug, PartialEq, Eq)]
enum QueryField {
    Any,
    Alias,
    Path,
    Tag,
    Note,
    Id,
    Missing,
    Dupe,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct QueryTerm {
    field: QueryField,
    value: String,
    negated: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct QuerySpec {
    terms: Vec<QueryTerm>,
}

struct DuplicateIndex {
    path_dupes: HashSet<String>,
    alias_dupes: HashSet<String>,
}

fn parse_query(input: &str) -> Result<QuerySpec> {
    let mut terms = Vec::new();
    for raw_token in tokenize_query(input)? {
        let (negated, token) = if let Some(rest) = raw_token.strip_prefix('-') {
            (true, rest)
        } else {
            (false, raw_token.as_str())
        };
        if token.is_empty() {
            bail!("A negation must include a query term. Example: -tag:archive");
        }

        let (field, value) = if let Some((field, value)) = token.split_once(':') {
            let field = match field.to_ascii_lowercase().as_str() {
                "alias" => QueryField::Alias,
                "path" => QueryField::Path,
                "tag" => QueryField::Tag,
                "note" => QueryField::Note,
                "id" => QueryField::Id,
                "missing" => QueryField::Missing,
                "dupe" => QueryField::Dupe,
                _ => bail!(
                    "Unsupported query field '{field}'. Try alias:, path:, tag:, note:, id:, missing:, or dupe:."
                ),
            };
            (field, value.trim().to_lowercase())
        } else {
            (QueryField::Any, token.trim().to_lowercase())
        };

        if value.is_empty() || value == "=" {
            bail!("Query terms cannot be empty. Example: tag:work");
        }

        match field {
            QueryField::Id => {
                value
                    .trim_start_matches('=')
                    .parse::<u64>()
                    .with_context(|| format!("Invalid id query '{value}'. Example: id:3"))?;
            }
            QueryField::Missing | QueryField::Dupe => {
                parse_bool_term(value.trim_start_matches('='))?;
            }
            _ => {}
        }

        terms.push(QueryTerm {
            field,
            value,
            negated,
        });
    }
    if terms.is_empty() {
        bail!("Query cannot be empty. Example: tag:work");
    }
    Ok(QuerySpec { terms })
}

fn tokenize_query(input: &str) -> Result<Vec<String>> {
    let mut tokens = Vec::new();
    let mut token = String::new();
    let mut quote = None;
    let mut escaped = false;
    for ch in input.chars() {
        if escaped {
            token.push(ch);
            escaped = false;
            continue;
        }
        if ch == '\\' {
            escaped = true;
            continue;
        }
        if let Some(delimiter) = quote {
            if ch == delimiter {
                quote = None;
            } else {
                token.push(ch);
            }
        } else if ch == '\'' || ch == '"' {
            quote = Some(ch);
        } else if ch.is_whitespace() {
            if !token.is_empty() {
                tokens.push(std::mem::take(&mut token));
            }
        } else {
            token.push(ch);
        }
    }
    if escaped || quote.is_some() {
        bail!("Unterminated quote or escape in query");
    }
    if !token.is_empty() {
        tokens.push(token);
    }
    Ok(tokens)
}

fn query_text_matches(candidate: &str, value: &str) -> bool {
    let candidate = candidate.to_lowercase();
    if let Some(exact) = value.strip_prefix('=') {
        candidate == exact
    } else {
        candidate.contains(value)
    }
}

fn parse_bool_term(value: &str) -> Result<bool> {
    match value {
        "true" => Ok(true),
        "false" => Ok(false),
        _ => bail!("Expected true or false, got '{value}'"),
    }
}

fn duplicate_index(store: &Store) -> DuplicateIndex {
    let mut path_counts: HashMap<String, usize> = HashMap::new();
    let mut alias_counts: HashMap<String, usize> = HashMap::new();

    for item in &store.items {
        *path_counts.entry(item.path.clone()).or_default() += 1;
        if let Some(alias) = item.alias.as_deref() {
            *alias_counts.entry(normalize_alias_key(alias)).or_default() += 1;
        }
    }

    DuplicateIndex {
        path_dupes: path_counts
            .into_iter()
            .filter_map(|(path, count)| (count > 1).then_some(path))
            .collect(),
        alias_dupes: alias_counts
            .into_iter()
            .filter_map(|(alias, count)| (count > 1).then_some(alias))
            .collect(),
    }
}

fn normalize_alias_key(alias: &str) -> String {
    alias
        .chars()
        .filter(|ch| ch.is_ascii_alphanumeric())
        .flat_map(|ch| ch.to_lowercase())
        .collect()
}

fn is_duplicate_item(item: &Favorite, duplicates: &DuplicateIndex) -> bool {
    duplicates.path_dupes.contains(item.path.as_str())
        || item
            .alias
            .as_deref()
            .map(normalize_alias_key)
            .is_some_and(|key| duplicates.alias_dupes.contains(&key))
}

fn matches_query_spec(item: &Favorite, query: &QuerySpec, duplicates: &DuplicateIndex) -> bool {
    query.terms.iter().all(|term| {
        let matched = match term.field {
            QueryField::Any => {
                query_text_matches(&item.path, &term.value)
                    || item
                        .alias
                        .as_deref()
                        .is_some_and(|v| query_text_matches(v, &term.value))
                    || item
                        .note
                        .as_deref()
                        .is_some_and(|v| query_text_matches(v, &term.value))
                    || item.tags.iter().any(|v| query_text_matches(v, &term.value))
            }
            QueryField::Alias => item
                .alias
                .as_ref()
                .is_some_and(|alias| query_text_matches(alias, &term.value)),
            QueryField::Path => query_text_matches(&item.path, &term.value),
            QueryField::Tag => item
                .tags
                .iter()
                .any(|tag| query_text_matches(tag, &term.value)),
            QueryField::Note => item
                .note
                .as_ref()
                .is_some_and(|note| query_text_matches(note, &term.value)),
            QueryField::Id => item.id.to_string() == term.value.trim_start_matches('='),
            QueryField::Missing => {
                Path::new(item.path.as_str()).exists()
                    != parse_bool_term(term.value.trim_start_matches('=')).unwrap_or(false)
            }
            QueryField::Dupe => {
                is_duplicate_item(item, duplicates)
                    == parse_bool_term(term.value.trim_start_matches('=')).unwrap_or(false)
            }
        };
        if term.negated { !matched } else { matched }
    })
}

fn smart_score(item: &Favorite, filters: &Filters, query_hint: Option<&str>) -> i64 {
    let mut score = (item.uses as i64) * 20 + item.last_used.unwrap_or(0) / 86_400;

    let mut needles: Vec<QueryTerm> = Vec::new();
    if let Some(search) = filters.search.as_deref() {
        needles.push(QueryTerm {
            field: QueryField::Any,
            value: search.to_ascii_lowercase(),
            negated: false,
        });
    }
    if let Some(hint) = query_hint {
        needles.push(QueryTerm {
            field: QueryField::Any,
            value: hint.to_ascii_lowercase(),
            negated: false,
        });
    }
    if let Some(query) = filters.query.as_ref() {
        needles.extend(
            query
                .terms
                .iter()
                .filter(|term| !term.negated)
                .filter(|term| {
                    !matches!(
                        term.field,
                        QueryField::Id | QueryField::Missing | QueryField::Dupe
                    )
                })
                .cloned(),
        );
    }

    for term in needles {
        let term = QueryTerm {
            value: term.value.trim_start_matches('=').to_string(),
            ..term
        };
        score += match term.field {
            QueryField::Alias => rank_text(item.alias.as_deref(), &term.value, 1500),
            QueryField::Path => rank_text(Some(item.path.as_str()), &term.value, 1300),
            QueryField::Tag => item
                .tags
                .iter()
                .map(|tag| rank_text(Some(tag.as_str()), &term.value, 1200))
                .max()
                .unwrap_or(0),
            QueryField::Note => rank_text(item.note.as_deref(), &term.value, 1250),
            QueryField::Any => [
                rank_text(item.alias.as_deref(), &term.value, 2000),
                rank_text(Some(item.path.as_str()), &term.value, 1500),
                rank_text(item.note.as_deref(), &term.value, 1400),
                item.tags
                    .iter()
                    .map(|tag| rank_text(Some(tag.as_str()), &term.value, 1200))
                    .max()
                    .unwrap_or(0),
            ]
            .into_iter()
            .max()
            .unwrap_or(0),
            QueryField::Id | QueryField::Missing | QueryField::Dupe => 0,
        };
    }

    score
}

fn rank_text(candidate: Option<&str>, needle: &str, exact_bonus: i64) -> i64 {
    let Some(candidate) = candidate else {
        return 0;
    };
    let candidate = candidate.to_ascii_lowercase();
    if candidate == needle {
        exact_bonus
    } else if candidate.starts_with(needle) {
        exact_bonus - 400
    } else if candidate.contains(needle) {
        exact_bonus - 900
    } else {
        0
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
        bail!("Rename must be in the form old=new (example: --rename-tag old=new)");
    }
    Ok((old, new))
}

#[cfg(not(feature = "coverage"))]
fn pick_item(store: &Store, filters: &Filters, opts: &ListOptions) -> Result<Option<u64>> {
    let mut items = filtered_items(store, filters);
    sort_items(
        &mut items,
        opts.sort,
        opts.reverse,
        filters,
        opts.smart,
        None,
    );
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
    sort_items(
        &mut items,
        opts.sort,
        opts.reverse,
        filters,
        opts.smart,
        None,
    );
    Ok(items.first().map(|item| item.id))
}

fn tui_items<'a>(
    store: &'a Store,
    filters: &Filters,
    sort: SortBy,
    reverse: bool,
    smart: bool,
    filter: &str,
) -> Vec<&'a Favorite> {
    let mut items = filtered_items(store, filters);
    if !filter.is_empty() {
        let filter_lower = filter.to_lowercase();
        items.retain(|item| matches_text_query(item, filter_lower.as_str()));
    }
    sort_items(
        &mut items,
        sort,
        reverse,
        filters,
        smart,
        (!filter.is_empty()).then_some(filter),
    );
    items
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
        query: args.query.as_deref().map(parse_query).transpose()?,
    };
    let list_opts = ListOptions {
        format: OutputFormat::Plain,
        path_format: args.display_path_format,
        sort: args.sort,
        reverse: args.reverse,
        smart: args.smart,
        show_notes: false,
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
        let items = tui_items(
            store,
            &base_filters,
            list_opts.sort,
            list_opts.reverse,
            list_opts.smart,
            filter.as_str(),
        );

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
                    let items = tui_items(
                        store,
                        &base_filters,
                        list_opts.sort,
                        list_opts.reverse,
                        list_opts.smart,
                        filter.as_str(),
                    );
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
        query: args.query.as_deref().map(parse_query).transpose()?,
    };
    let opts = ListOptions {
        format: OutputFormat::Plain,
        path_format: args.display_path_format,
        sort: args.sort,
        reverse: args.reverse,
        smart: args.smart,
        show_notes: false,
    };
    let items = tui_items(store, &filters, opts.sort, opts.reverse, opts.smart, "");
    Ok(items.first().map(|item| item.id))
}

fn export_store(store: &Store, file: Option<PathBuf>) -> Result<()> {
    let data = serde_json::to_vec_pretty(store).context("serialize store")?;
    match file {
        Some(path) => {
            if automation::is_dry_run() {
                println!("Would export to {}", path.display());
                return Ok(());
            }
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
    if data.trim().is_empty() {
        bail!("No input provided. Use --file <path> or pipe JSON to stdin.");
    }
    let mut store: Store =
        serde_json::from_str(&data).context("parse store (expected fav JSON)")?;
    validate_store(&store)?;
    sync_store(&mut store);
    Ok(store)
}

fn merge_store(existing: &mut Store, mut imported: Store) -> Result<()> {
    let aliases: Vec<String> = existing
        .items
        .iter()
        .filter_map(|item| item.alias.clone())
        .collect();
    let preset_names: HashSet<String> = existing
        .presets
        .iter()
        .map(|preset| preset.name.clone())
        .collect();
    for item in imported.items.iter() {
        if let Some(alias) = item.alias.as_ref()
            && aliases.iter().any(|a| a == alias)
        {
            bail!("Alias conflict during merge: {alias}. Rename it before importing.");
        }
    }
    for preset in imported.presets.iter() {
        if preset_names.contains(&preset.name) {
            bail!(
                "Preset conflict during merge: {}. Rename it before importing.",
                preset.name
            );
        }
    }
    let mut next_id = existing.next_id.max(1);
    for mut item in imported.items.drain(..) {
        item.id = next_id;
        next_id += 1;
        existing.items.push(item);
    }
    existing.presets.append(&mut imported.presets);
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

fn build_command_parts(command: &[String], path: &str) -> Result<(String, Vec<String>)> {
    let program = command
        .first()
        .cloned()
        .context("Provide a command after -- (example: fav with <target> -- ls -la)")?;
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
    Ok((program, args))
}

fn render_command_line(program: &str, args: &[String]) -> String {
    std::iter::once(program.to_string())
        .chain(args.iter().cloned())
        .map(|part| shell_escape(&part))
        .collect::<Vec<_>>()
        .join(" ")
}

fn shell_escape(value: &str) -> String {
    if value.chars().all(|ch| {
        ch.is_ascii_alphanumeric() || matches!(ch, '/' | '-' | '_' | '.' | ':' | '=' | '~')
    }) {
        value.to_string()
    } else {
        format!("'{}'", value.replace('\'', "'\"'\"'"))
    }
}

fn run_with_command(command: &[String], path: &str, dry_run: bool) -> Result<()> {
    let (program, args) = build_command_parts(command, path)?;
    automation::data(serde_json::json!({"program": program, "args": args}));
    if dry_run {
        println!("{}", render_command_line(&program, &args));
        return Ok(());
    }

    automation::unlock();
    let mut cmd = ProcessCommand::new(program);
    if automation::is_json() {
        let output = cmd.args(args).output().context("run command")?;
        automation::data(serde_json::json!({"exit_code": output.status.code(),
            "stdout": String::from_utf8_lossy(&output.stdout), "stderr": String::from_utf8_lossy(&output.stderr)}));
        if !output.status.success() {
            return Err(automation::Failure {
                code: "execution_failed",
                exit: 7,
                message: format!(
                    "Command exited with status {}: {}",
                    output.status,
                    String::from_utf8_lossy(&output.stderr)
                ),
            }
            .into());
        }
        Ok(())
    } else {
        let status = cmd.args(args).status().context("run command")?;
        std::process::exit(status.code().unwrap_or(1));
    }
}

fn render_shell_init(shell: ShellKind) -> String {
    match shell {
        ShellKind::Zsh | ShellKind::Bash => r#"fcd() {
  if [ "$#" -lt 1 ]; then
    echo "usage: fcd <target>" >&2
    return 1
  fi
  local dest
  dest="$(fav get "$1")" || return $?
  cd "$dest"
}

fopen() {
  fav open "$@"
}

frun() {
  if [ "$#" -lt 2 ]; then
    echo "usage: frun <preset> <target>" >&2
    return 1
  fi
  local preset="$1"
  shift
  fav preset run "$preset" "$@"
}
"#
        .to_string(),
        ShellKind::Fish => r#"function fcd
    if test (count $argv) -lt 1
        echo "usage: fcd <target>" >&2
        return 1
    end
    set dest (fav get $argv[1]); or return $status
    cd $dest
end

function fopen
    fav open $argv
end

function frun
    if test (count $argv) -lt 2
        echo "usage: frun <preset> <target>" >&2
        return 1
    end
    set preset $argv[1]
    set -e argv[1]
    fav preset run $preset $argv
end
"#
        .to_string(),
    }
}

fn opener_program() -> Result<&'static str> {
    match env::consts::OS {
        "macos" => Ok("open"),
        "linux" => Ok("xdg-open"),
        "windows" => Ok("explorer.exe"),
        other => bail!("No supported opener for platform '{other}'."),
    }
}

fn render_open_command(path: &str) -> Result<String> {
    Ok(render_command_line(opener_program()?, &[path.to_string()]))
}

fn run_open_command(path: &str) -> Result<()> {
    if automation::is_json() {
        let output = ProcessCommand::new(opener_program()?)
            .arg(path)
            .output()
            .context("run opener")?;
        automation::data(
            serde_json::json!({"exit_code": output.status.code(), "stdout": String::from_utf8_lossy(&output.stdout), "stderr": String::from_utf8_lossy(&output.stderr)}),
        );
        if !output.status.success() {
            bail!("Open command failed for {path}");
        }
        return Ok(());
    }
    let status = ProcessCommand::new(opener_program()?)
        .arg(path)
        .status()
        .context("run opener")?;
    if !status.success() {
        bail!("Open command failed for {path}");
    }
    Ok(())
}

fn validate_preset_name(name: &str) -> Result<()> {
    if name.trim().is_empty() {
        bail!("Preset name cannot be empty");
    }
    Ok(())
}

fn ensure_unique_preset(store: &Store, name: &str) -> Result<()> {
    if store.presets.iter().any(|preset| preset.name == name) {
        bail!("Preset already exists: {name}");
    }
    Ok(())
}

fn find_preset<'a>(store: &'a Store, name: &str) -> Result<&'a Preset> {
    store
        .presets
        .iter()
        .find(|preset| preset.name == name)
        .with_context(|| format!("No preset named '{name}'."))
}

fn report_duplicates(store: &Store) -> Result<()> {
    let duplicates = duplicate_index(store);
    for item in &store.items {
        if duplicates.path_dupes.contains(item.path.as_str()) {
            println!(
                "path\t{}\t{}\t{}",
                item.id,
                item.alias.as_deref().unwrap_or("-"),
                item.path
            );
        }
        if item
            .alias
            .as_deref()
            .map(normalize_alias_key)
            .is_some_and(|alias| duplicates.alias_dupes.contains(&alias))
        {
            println!(
                "alias\t{}\t{}\t{}",
                item.id,
                item.alias.as_deref().unwrap_or("-"),
                item.path
            );
        }
    }
    Ok(())
}

fn repair_paths(store: &mut Store, from: &Path, to: &Path, dry_run: bool) -> Result<usize> {
    let from = expand_and_absolute(from)?;
    let to = expand_and_absolute(to)?;
    let mut repaired = 0;

    for item in &mut store.items {
        let path = PathBuf::from(&item.path);
        if path.exists() {
            continue;
        }
        if let Ok(suffix) = path.strip_prefix(&from) {
            let candidate = to.join(suffix);
            if candidate.exists() {
                let repaired_path = candidate
                    .canonicalize()
                    .context("canonicalize repaired path")?;
                println!("{}\t{}", item.path, repaired_path.to_string_lossy());
                if !dry_run {
                    item.path = repaired_path.to_string_lossy().to_string();
                }
                repaired += 1;
            }
        }
    }

    Ok(repaired)
}

struct ImportReport {
    added: usize,
    skipped: usize,
}

fn import_history(store: &mut Store, args: &ImportHistoryArgs) -> Result<ImportReport> {
    let (shell, data) = resolve_history_source(args)?;
    let mut commands = extract_history_commands(&data, shell);
    if let Some(limit) = args.limit {
        commands = commands.into_iter().rev().take(limit).collect::<Vec<_>>();
        commands.reverse();
    }
    let paths = commands
        .into_iter()
        .flat_map(|command| extract_paths_from_command(&command))
        .collect::<Vec<_>>();
    import_paths(
        store,
        paths,
        &normalize_tags(args.tag.clone()),
        args.note.as_deref(),
    )
}

fn import_paths_file(store: &mut Store, args: &ImportPathsArgs) -> Result<ImportReport> {
    let data = read_optional_file_or_stdin(args.file.as_deref())?;
    let paths = parse_path_input(&data)?;
    import_paths(
        store,
        paths,
        &normalize_tags(args.tag.clone()),
        args.note.as_deref(),
    )
}

fn import_paths(
    store: &mut Store,
    raw_paths: Vec<String>,
    tags: &[String],
    note: Option<&str>,
) -> Result<ImportReport> {
    let mut report = ImportReport {
        added: 0,
        skipped: 0,
    };
    let mut seen = HashSet::new();

    for raw_path in raw_paths {
        let trimmed = raw_path.trim();
        if trimmed.is_empty() || !seen.insert(trimmed.to_string()) {
            report.skipped += 1;
            continue;
        }
        if add_imported_path(store, trimmed, tags, note)? {
            report.added += 1;
        } else {
            report.skipped += 1;
        }
    }

    sync_store(store);
    Ok(report)
}

fn add_imported_path(
    store: &mut Store,
    raw_path: &str,
    tags: &[String],
    note: Option<&str>,
) -> Result<bool> {
    let normalized = match normalize_path(Path::new(raw_path)) {
        Ok(path) => path,
        Err(_) => return Ok(false),
    };
    let normalized = normalized.to_string_lossy().to_string();
    if store.items.iter().any(|item| item.path == normalized) {
        return Ok(false);
    }

    let id = store.next_id.max(1);
    store.next_id = id + 1;
    store.items.push(Favorite {
        id,
        path: normalized,
        alias: None,
        tags: tags.to_vec(),
        note: note.map(str::to_string),
        uses: 0,
        last_used: None,
    });
    Ok(true)
}

fn read_optional_file_or_stdin(file: Option<&Path>) -> Result<String> {
    let mut data = String::new();
    match file {
        Some(path) => {
            data = fs::read_to_string(path).with_context(|| format!("read {}", path.display()))?;
        }
        None => {
            io::stdin()
                .read_to_string(&mut data)
                .context("read stdin")?;
        }
    }
    if data.trim().is_empty() {
        bail!("No input provided. Use --file <path> or pipe data to stdin.");
    }
    Ok(data)
}

fn resolve_history_source(args: &ImportHistoryArgs) -> Result<(HistoryShell, String)> {
    if let Some(path) = args.file.as_deref() {
        let data = fs::read_to_string(path).with_context(|| format!("read {}", path.display()))?;
        let shell = if args.shell == HistoryShell::Auto {
            detect_history_shell(Some(path), &data)
        } else {
            args.shell
        };
        return Ok((shell, data));
    }

    let user_dirs = UserDirs::new().context("resolve home directory")?;
    let home = user_dirs.home_dir();
    let candidates = match args.shell {
        HistoryShell::Auto => vec![
            (HistoryShell::Zsh, home.join(".zsh_history")),
            (HistoryShell::Bash, home.join(".bash_history")),
            (
                HistoryShell::Fish,
                home.join(".local/share/fish/fish_history"),
            ),
        ],
        HistoryShell::Zsh => vec![(HistoryShell::Zsh, home.join(".zsh_history"))],
        HistoryShell::Bash => vec![(HistoryShell::Bash, home.join(".bash_history"))],
        HistoryShell::Fish => vec![(
            HistoryShell::Fish,
            home.join(".local/share/fish/fish_history"),
        )],
    };

    for (shell, path) in candidates {
        if path.exists() {
            let data =
                fs::read_to_string(&path).with_context(|| format!("read {}", path.display()))?;
            return Ok((shell, data));
        }
    }

    bail!("Could not find a shell history file. Use --file to point at one.");
}

fn detect_history_shell(path: Option<&Path>, data: &str) -> HistoryShell {
    if let Some(path) = path
        .and_then(|path| path.file_name())
        .and_then(|name| name.to_str())
    {
        let lower = path.to_ascii_lowercase();
        if lower.contains("zsh") {
            return HistoryShell::Zsh;
        }
        if lower.contains("fish") {
            return HistoryShell::Fish;
        }
        if lower.contains("bash") {
            return HistoryShell::Bash;
        }
    }
    if data.lines().any(|line| line.starts_with(": ")) {
        HistoryShell::Zsh
    } else if data
        .lines()
        .any(|line| line.trim_start().starts_with("- cmd:"))
    {
        HistoryShell::Fish
    } else {
        HistoryShell::Bash
    }
}

fn extract_history_commands(data: &str, shell: HistoryShell) -> Vec<String> {
    data.lines()
        .filter_map(|line| match shell {
            HistoryShell::Auto => None,
            HistoryShell::Zsh => line.split_once(';').map(|(_, command)| command.to_string()),
            HistoryShell::Bash => Some(line.to_string()),
            HistoryShell::Fish => line
                .trim_start()
                .strip_prefix("- cmd:")
                .map(|command| command.trim().to_string()),
        })
        .collect()
}

fn extract_paths_from_command(command: &str) -> Vec<String> {
    let tokens = split_command_tokens(command);
    if tokens.is_empty() {
        return Vec::new();
    }
    if tokens[0] == "cd" && tokens.len() > 1 {
        return vec![tokens[1].clone()];
    }
    tokens
        .into_iter()
        .filter(|token| looks_like_path(token))
        .collect()
}

fn split_command_tokens(command: &str) -> Vec<String> {
    let mut tokens = Vec::new();
    let mut current = String::new();
    let mut quote: Option<char> = None;

    for ch in command.chars() {
        match (quote, ch) {
            (Some(active), c) if c == active => quote = None,
            (Some(_), c) => current.push(c),
            (None, '\'' | '"') => quote = Some(ch),
            (None, c) if c.is_whitespace() => {
                if !current.is_empty() {
                    tokens.push(current.clone());
                    current.clear();
                }
            }
            (None, c) => current.push(c),
        }
    }
    if !current.is_empty() {
        tokens.push(current);
    }
    tokens
}

fn looks_like_path(value: &str) -> bool {
    value.starts_with('/')
        || value.starts_with("~/")
        || value.starts_with("./")
        || value.starts_with("../")
}

fn parse_path_input(data: &str) -> Result<Vec<String>> {
    if let Ok(paths) = serde_json::from_str::<Vec<String>>(data) {
        return Ok(paths);
    }
    if data.contains("HREF=\"file://") || data.contains("href=\"file://") {
        return Ok(extract_bookmark_paths(data));
    }

    let mut paths = Vec::new();
    for line in data.lines() {
        for cell in line.split(',') {
            let trimmed = cell.trim().trim_matches('"').trim_matches('\'');
            if !trimmed.is_empty() {
                paths.push(trimmed.to_string());
            }
        }
    }
    Ok(paths)
}

fn extract_bookmark_paths(data: &str) -> Vec<String> {
    let mut paths = Vec::new();
    for marker in ["HREF=\"file://", "href=\"file://"] {
        for segment in data.split(marker).skip(1) {
            if let Some((value, _)) = segment.split_once('"') {
                paths.push(decode_file_url(value));
            }
        }
    }
    paths
}

fn decode_file_url(value: &str) -> String {
    let path = value.trim_start_matches('/');
    format!("/{}", path.replace("%20", " "))
}

#[cfg(test)]
#[allow(clippy::items_after_test_module)]
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
                    note: None,
                    uses: 2,
                    last_used: Some(100),
                },
                Favorite {
                    id: 2,
                    path: "/var/log/syslog".to_string(),
                    alias: None,
                    tags: vec!["ops".to_string()],
                    note: None,
                    uses: 5,
                    last_used: Some(200),
                },
                Favorite {
                    id: 3,
                    path: "/home/user/docs".to_string(),
                    alias: Some("docs".to_string()),
                    tags: vec!["work".to_string()],
                    note: Some("Project notes".to_string()),
                    uses: 1,
                    last_used: Some(50),
                },
            ],
            presets: Vec::new(),
        }
    }

    #[test]
    fn filters_by_tag() {
        let store = sample_store();
        let filters = Filters {
            search: None,
            tags: vec!["work".to_string()],
            query: None,
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
            query: None,
        };
        let items = filtered_items(&store, &filters);
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].id, 2);
    }

    #[test]
    fn parse_query_supports_free_text_fields_and_negation() {
        let query = parse_query("docs tag:work -tag:notes note:project id:3").expect("parse");
        assert_eq!(query.terms.len(), 5);
        assert_eq!(query.terms[0].value, "docs");
        assert_eq!(query.terms[1].field, QueryField::Tag);
        assert!(query.terms[2].negated);
        assert_eq!(query.terms[3].field, QueryField::Note);
        assert_eq!(query.terms[4].field, QueryField::Id);
    }

    #[test]
    fn parse_query_rejects_empty_input() {
        assert!(parse_query("   ").is_err());
    }

    #[test]
    fn filters_by_structured_query() {
        let store = sample_store();
        let filters = Filters {
            search: None,
            tags: Vec::new(),
            query: Some(parse_query("tag:work note:project").expect("query")),
        };
        let items = filtered_items(&store, &filters);
        let ids: Vec<u64> = items.into_iter().map(|item| item.id).collect();
        assert_eq!(ids, vec![3]);
    }

    #[test]
    fn smart_sort_prefers_exact_alias_matches() {
        let store = sample_store();
        let filters = Filters {
            search: Some("docs".to_string()),
            tags: Vec::new(),
            query: None,
        };
        let mut items: Vec<&Favorite> = store.items.iter().collect();
        sort_items(&mut items, SortBy::Id, false, &filters, true, None);
        let ids: Vec<u64> = items.into_iter().map(|item| item.id).collect();
        assert_eq!(ids[0], 3);
    }

    #[test]
    fn tui_items_respect_smart_flag() {
        let store = Store {
            version: STORE_VERSION,
            next_id: 3,
            items: vec![
                Favorite {
                    id: 1,
                    path: "/tmp/docs-exact".to_string(),
                    alias: Some("docs".to_string()),
                    tags: Vec::new(),
                    note: None,
                    uses: 0,
                    last_used: Some(10),
                },
                Favorite {
                    id: 2,
                    path: "/tmp/docs-recent".to_string(),
                    alias: Some("recent".to_string()),
                    tags: Vec::new(),
                    note: Some("docs backup".to_string()),
                    uses: 0,
                    last_used: Some(100),
                },
            ],
            presets: Vec::new(),
        };
        let filters = Filters {
            search: None,
            tags: Vec::new(),
            query: None,
        };

        let plain_ids: Vec<u64> = tui_items(&store, &filters, SortBy::Recent, false, false, "docs")
            .into_iter()
            .map(|item| item.id)
            .collect();
        assert_eq!(plain_ids, vec![2, 1]);

        let smart_ids: Vec<u64> = tui_items(&store, &filters, SortBy::Recent, false, true, "docs")
            .into_iter()
            .map(|item| item.id)
            .collect();
        assert_eq!(smart_ids, vec![1, 2]);
    }

    #[test]
    fn sort_by_recent_desc() {
        let store = sample_store();
        let mut items: Vec<&Favorite> = store.items.iter().collect();
        let filters = Filters {
            search: None,
            tags: Vec::new(),
            query: None,
        };
        sort_items(&mut items, SortBy::Recent, false, &filters, false, None);
        let ids: Vec<u64> = items.into_iter().map(|item| item.id).collect();
        assert_eq!(ids, vec![2, 1, 3]);
    }

    #[test]
    fn sort_by_alias_asc() {
        let store = sample_store();
        let mut items: Vec<&Favorite> = store.items.iter().collect();
        let filters = Filters {
            search: None,
            tags: Vec::new(),
            query: None,
        };
        sort_items(&mut items, SortBy::Alias, false, &filters, false, None);
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
                note: None,
                uses: 0,
                last_used: None,
            }],
            presets: Vec::new(),
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
                note: None,
                uses: 0,
                last_used: None,
            }],
            presets: Vec::new(),
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
                note: None,
                uses: 0,
                last_used: None,
            }],
            presets: Vec::new(),
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
    fn old_store_json_loads_defaults_for_notes_and_presets() {
        let data = r#"{
            "version": 1,
            "next_id": 2,
            "items": [
                {
                    "id": 1,
                    "path": "/tmp/alpha",
                    "alias": "alpha",
                    "tags": ["work"],
                    "uses": 0,
                    "last_used": null
                }
            ]
        }"#;

        let store: Store = serde_json::from_str(data).expect("parse");
        assert!(store.presets.is_empty());
        assert!(store.items[0].note.is_none());
    }

    #[test]
    fn save_and_load_store_roundtrip() {
        let dir = tempdir().expect("tempdir");
        let config = dir.path().join("nested/fav.json");
        let mut store = sample_store();
        store.items[0].note = Some("Primary work tree".to_string());
        store.presets.push(Preset {
            name: "show".to_string(),
            command: vec!["printf".to_string(), "%s".to_string(), "{}".to_string()],
            note: Some("Minimal smoke test preset".to_string()),
        });
        save_store(&config, &store).expect("save");
        let loaded = load_store(&config).expect("load");
        assert_eq!(loaded.items.len(), store.items.len());
        assert_eq!(loaded.items[0].path, store.items[0].path);
        assert_eq!(loaded.items[0].note, store.items[0].note);
        assert_eq!(loaded.presets.len(), 1);
        assert_eq!(loaded.presets[0].name, "show");
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
                note: None,
                uses: 0,
                last_used: None,
            }],
            presets: Vec::new(),
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
        let filters = Filters {
            search: None,
            tags: Vec::new(),
            query: None,
        };
        sort_items(&mut by_path, SortBy::Path, false, &filters, false, None);
        let path_ids: Vec<u64> = by_path.iter().map(|item| item.id).collect();
        assert_eq!(path_ids, vec![3, 1, 2]);

        let mut by_tag: Vec<&Favorite> = store.items.iter().collect();
        sort_items(&mut by_tag, SortBy::Tag, false, &filters, false, None);
        assert_eq!(by_tag.first().unwrap().id, 2);

        let mut by_uses: Vec<&Favorite> = store.items.iter().collect();
        sort_items(&mut by_uses, SortBy::Uses, false, &filters, false, None);
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
                note: None,
                uses: 0,
                last_used: None,
            }],
            presets: Vec::new(),
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
    let canonical = absolute.canonicalize().with_context(|| {
        format!(
            "Path does not exist: {}. Provide an existing path.",
            absolute.display()
        )
    })?;
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
            .with_context(|| format!("No favorite with id {id}. Run `fav list` to see ids."));
    }
    if let Some(idx) = store
        .items
        .iter()
        .position(|item| item.alias.as_deref() == Some(target))
    {
        return Ok(idx);
    }
    let normalized = normalize_path_lenient(Path::new(target))?;
    let normalized = normalized.to_string_lossy().to_string();
    if let Some(idx) = store
        .items
        .iter()
        .position(|item| item.path == normalized || item.path == target)
    {
        return Ok(idx);
    }
    bail!(
        "No favorite matches target '{target}' (normalized to {normalized}). Run `fav list` or `fav add`."
    )
}

fn resolve_target_indices(
    store: &Store,
    target: Option<&str>,
    query: Option<&str>,
) -> Result<Vec<usize>> {
    match (target, query) {
        (Some(target), None) => Ok(vec![resolve_target_index(store, target)?]),
        (None, Some(query)) => {
            let filters = Filters {
                search: None,
                tags: Vec::new(),
                query: Some(parse_query(query)?),
            };
            let matched_ids: Vec<u64> = filtered_items(store, &filters)
                .into_iter()
                .map(|item| item.id)
                .collect();
            if matched_ids.is_empty() {
                bail!("No favorites matched query '{query}'.");
            }
            let mut indices = matched_ids
                .into_iter()
                .filter_map(|id| store.items.iter().position(|item| item.id == id))
                .collect::<Vec<_>>();
            indices.sort_unstable();
            Ok(indices)
        }
        (Some(_), Some(_)) => bail!("Choose only one of: target or --query"),
        (None, None) => bail!("Provide a target or --query. See `fav --help`."),
    }
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
        bail!("Alias already in use: {alias}");
    }
    Ok(())
}

fn ensure_unique_alias_except(store: &Store, alias: &str, except_idx: usize) -> Result<()> {
    if store
        .items
        .iter()
        .enumerate()
        .any(|(idx, item)| idx != except_idx && item.alias.as_deref() == Some(alias))
    {
        bail!("Alias already in use: {alias}");
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
