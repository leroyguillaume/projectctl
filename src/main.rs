mod cfg;
mod cli;
mod cmd;
mod consts;
mod err;
mod fs;
mod git;
mod log;
mod renderer;

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
        CommandKind::Env(cmd) => cmd.run(&mut stdout),
        CommandKind::New(cmd) => cmd.run(),
    };
    let rc = match res {
        Ok(()) => exitcode::OK,
        Err(err) => {
            error!("{}", err);
            if let Error::InvalidConfig(ref errs) = err {
                for err in errs {
                    error!("{}", err);
                }
            }
            err.to_return_code()
        }
    };
    exit(rc);
}
