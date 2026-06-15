use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(
    name = "whetstone",
    version = env!("WHETSTONE_VERSION"),
    about = "Headroom + RTK + Memory for Claude Code"
)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Option<Command>,
}

#[derive(Subcommand)]
pub enum Command {
    /// Install Headroom, RTK, and a memory provider
    Setup {
        /// Force-upgrade all tools and refresh installed files
        #[arg(long)]
        full: bool,

        /// Headroom pip extras: "all" (default), "none", or
        /// comma-separated like "proxy,code"
        #[arg(long, default_value = "all")]
        headroom_extras: String,
    },

    /// Remove whetstone components
    Uninstall,

    /// Start Claude Code via headroom wrap (default when no subcommand given)
    Claude {
        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
        args: Vec<String>,
    },

    /// Alias for claude
    Code {
        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
        args: Vec<String>,
    },

    /// Run headroom proxy
    Proxy {
        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
        args: Vec<String>,
    },

    /// Run rtk
    Rtk {
        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
        args: Vec<String>,
    },

    /// Print whetstone version
    Version,

    /// Interactive dashboard
    Dashboard,

    /// Modify project settings interactively
    Settings,

    /// Inspect ~/.claude/settings.json and report problems (Phase 1 task 1.2)
    Doctor,

    /// Migrate a v2 install to v3 (Phase 3)
    Migrate {
        /// Print the plan without writing anything
        #[arg(long)]
        dry_run: bool,

        /// Skip the interactive confirmation
        #[arg(long, short = 'y')]
        yes: bool,

        /// Restore an earlier migration by id (e.g. 20260607-153012)
        #[arg(long, value_name = "MIGRATION_ID")]
        rollback: Option<String>,
    },

    /// Pull latest and rerun setup
    Update {
        #[arg(long)]
        full: bool,
    },

    /// Prepare a release PR; GitHub Actions publishes after merge
    Release {
        #[command(subcommand)]
        action: ReleaseAction,
    },

    /// Deprecated legacy release path
    ReleasePublish {
        #[command(subcommand)]
        action: ReleaseAction,
    },

    /// Regenerate site/src/changelog.js from CHANGELOG.md
    ChangelogSync {
        /// Path to CHANGELOG.md (defaults to repo root)
        #[arg(long)]
        input: Option<String>,

        /// Output JS file (defaults to site/src/changelog.js)
        #[arg(long)]
        output: Option<String>,

        /// Max number of releases to include
        #[arg(long, default_value_t = 8)]
        limit: usize,
    },

    /// Show token savings across all whetstone components
    Stats,

    /// Session database operations
    Db {
        #[command(subcommand)]
        action: DbCommand,
    },
}

#[derive(Subcommand, Clone)]
pub enum ReleaseAction {
    Patch,
    Minor,
    Major,
    Set { version: String },
}

#[derive(Subcommand)]
pub enum DbCommand {
    /// Initialize or re-apply schema
    Init,

    /// Add a session diary entry (JSON argument)
    AddSession { json: String },

    /// Add an insight/decision (JSON argument)
    AddInsight { json: String },

    /// Full-text search across tables
    Search {
        query: String,
        #[arg(long)]
        project: Option<String>,
        #[arg(long, default_value_t = 10)]
        limit: usize,
    },

    /// Get recent sessions for a project
    GetSessions {
        project: String,
        #[arg(long, default_value_t = 5)]
        limit: usize,
    },

    /// Get insights for a project
    GetInsights { project: String },

    /// Get project context
    GetContext { project: String },

    /// Upsert project context (JSON argument)
    SetContext { json: String },

    /// Add a task to a project plan (JSON argument)
    AddPlanTask { json: String },

    /// Get all plan tasks for a project
    GetPlan { project: String },

    /// Update a plan task status (JSON argument)
    UpdateTask { json: String },

    /// Export project memory as markdown
    ExportMd { project: String },

    /// Show database statistics
    Stats,
}
