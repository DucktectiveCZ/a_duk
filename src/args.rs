use clap::{ArgAction, Parser};

#[derive(Parser, Debug)]
#[command(version = "1.0.0", about = "A duk.")]
pub struct Args {
    #[arg(short, long, default_value("duk"))]
    pub pet: String,
    #[arg(short, long, action(ArgAction::SetTrue), default_value("false"))]
    pub debug: bool,
}
