use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(
    name = "whetstone",
    version = env!("WHETSTONE_VERSION"),
    about = "Headroom + RTK + MemStack for Claude Code"
)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Option<Command>,
}

#[derive(Subcommand)]
pub enum Command {
    /// Install Headroom, RTK, and optionally MemStack
    Setup {
        /// Force-upgrade all tools and refresh MemStack files
        #[arg(long)]
        full: bool,

        /// Headroom pip extras: "all" (default), "none", or comma-separated like "proxy,code"
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

    /// Pull latest and rerun setup
    Update {
        #[arg(long)]
        full: bool,
    },

    /// Bump VERSION file
    Release {
        #[command(subcommand)]
        action: ReleaseAction,
    },

    /// Release, commit, tag, and push
    ReleasePublish {
        #[command(subcommand)]
        action: ReleaseAction,
    },

    /// MemStack database operations
    Db {
        #[command(subcommand)]
        action: DbCommand,
    },
}

#[derive(Subcommand, Clone)]
pub enum ReleaseAction {
    Patch {
        #[arg(long)]
        tag: bool,
    },
    Minor {
        #[arg(long)]
        tag: bool,
    },
    Major {
        #[arg(long)]
        tag: bool,
    },
    Set {
        version: String,
        #[arg(long)]
        tag: bool,
    },
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
