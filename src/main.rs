mod cli;
mod cmd;
mod err;
mod git;

use std::process::exit;

use clap::Parser;
use cli::Arguments;
use log::error;
use simple_logger::SimpleLogger;

fn main() {
    let args = Arguments::parse();
    SimpleLogger::new()
        .with_level(args.verbosity.log_level_filter())
        .init()
        .unwrap();
    let rc = match args.try_into_command().and_then(|cmd| cmd.run()) {
        Ok(()) => exitcode::OK,
        Err(err) => {
            error!("{}", err);
            err.to_return_code()
        }
    };
    exit(rc);
}
