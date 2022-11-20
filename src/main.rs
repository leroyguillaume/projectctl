mod cli;
mod git;

use clap::Parser;
use cli::Arguments;
use simple_logger::SimpleLogger;

fn main() {
    let args = Arguments::parse();
    SimpleLogger::new()
        .with_level(args.verbosity.log_level_filter())
        .init()
        .unwrap();
}
