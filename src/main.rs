mod cli;
mod cmd;
mod err;
mod fs;
mod git;
mod renderer;

use std::process::exit;

use clap::Parser;
use cli::Arguments;
use cmd::{Command, CommandKind};
use log::error;
use simple_logger::SimpleLogger;

fn main() {
    let args = Arguments::parse();
    SimpleLogger::new()
        .with_level(args.verbosity.log_level_filter())
        .init()
        .unwrap();
    let res = match args.into_command_kind() {
        CommandKind::New(cmd) => cmd.run(),
    };
    let rc = match res {
        Ok(()) => exitcode::OK,
        Err(err) => {
            error!("Error: {}", err);
            err.to_return_code()
        }
    };
    exit(rc);
}
