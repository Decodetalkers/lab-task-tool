use clap::{
    Parser,
    builder::{
        Styles,
        styling::{AnsiColor, Effects},
    },
};
fn get_styles() -> Styles {
    Styles::styled()
        .header(AnsiColor::Yellow.on_default() | Effects::BOLD)
        .usage(AnsiColor::Green.on_default() | Effects::BOLD)
        .literal(AnsiColor::Blue.on_default() | Effects::BOLD)
        .placeholder(AnsiColor::Green.on_default())
}

#[derive(Parser, Debug)]
#[command(version, about, styles=get_styles())]
pub struct Cli {
    #[arg(long)]
    pub task: String,
    #[arg(long)]
    pub log_file: Option<String>,
    #[arg(long, value_delimiter = ' ', num_args = 1..)]
    pub commands: Vec<String>,
}
