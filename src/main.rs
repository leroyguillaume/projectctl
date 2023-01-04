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
mod sys;

use std::{io::stdout, process::exit};

use crate::err::ErrorKind;

use self::log::Logger;
use ::log::error;
use clap::Parser;
use cli::Arguments;
use cmd::CommandKind;

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
            match err.kind {
                ErrorKind::InvalidConfig(ref errs) => {
                    for cause in errs {
                        error!("{}", cause);
                    }
                }
                ErrorKind::Liquid(ref err) => error!("{}", err),
                ErrorKind::ScriptFailed {
                    rc,
                    ref stderr,
                    ref stdout,
                } => {
                    if let Some(rc) = rc {
                        error!("Return code: {}", rc);
                    }
                    error!("stdout:\n{}", stdout);
                    error!("stderr:\n{}", stderr);
                }
                _ => (),
            }
            err.to_return_code()
        }
    };
    exit(rc);
}
