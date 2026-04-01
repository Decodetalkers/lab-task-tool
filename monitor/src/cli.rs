use clap::{Parser, Subcommand};

#[derive(Debug, Parser)] // requires `derive` feature
#[command(name = "lab-monitor")]
#[command(about = "monitor the tasks", long_about = None)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Debug, Subcommand)]
pub enum Commands {
    /// Clones repos
    Status,
    /// Compare two commits
    Restart,
    /// Reset failed task
    ResetFailed,
}
