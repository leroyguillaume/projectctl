mod cli;
mod cmd;
mod err;
mod fs;
mod git;
mod log;
mod renderer;

use std::process::exit;

use self::log::Logger;
use ::log::error;
use clap::Parser;
use cli::Arguments;
use cmd::{Command, CommandKind};

fn main() {
    let args = Arguments::parse();
    Logger::init(args.logging.to_level_filter(), !args.logging.no_color).unwrap();
    let res = match args.into_command_kind() {
        CommandKind::New(cmd) => cmd.run(),
    };
    let rc = match res {
        Ok(()) => exitcode::OK,
        Err(err) => {
            error!("{}", err);
            err.to_return_code()
        }
    };
    exit(rc);
}
