mod cfg;
mod cli;
mod cmd;
mod err;
mod fs;
mod git;
mod log;
mod paths;
mod renderer;
mod script;

use std::{io::stdout, process::exit};

use self::log::Logger;
use ::log::error;
use clap::Parser;
use cli::Arguments;
use cmd::CommandKind;
use err::Error;

fn main() {
    let args = Arguments::parse();
    Logger::init(args.logging.to_level_filter(), !args.logging.no_color).unwrap();
    let mut stdout = stdout();
    let res = match args.into_command_kind() {
        CommandKind::Destroy(cmd) => cmd.run(),
        CommandKind::Env(cmd) => cmd.run(&mut stdout),
        CommandKind::Hook(cmd) => cmd.run(&mut stdout),
        CommandKind::New(cmd) => cmd.run(),
    };
    let rc = match res {
        Ok(()) => exitcode::OK,
        Err(err) => {
            error!("{}", err);
            match err {
                Error::InvalidConfig { ref causes, .. } => {
                    for cause in causes {
                        error!("{}", cause);
                    }
                }
                Error::Liquid { ref cause, .. } => error!("{}", cause),
                _ => (),
            }
            err.to_return_code()
        }
    };
    exit(rc);
}
