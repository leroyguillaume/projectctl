use std::{io::stderr, process::exit};

use clap::Parser;
use cmd::{Args, CommandRunner};
use fs::DefaultFileSystem;
use model::ProjectctlResult;
use tracing::{error, Level};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt, EnvFilter};

fn main() {
    init_tracing();
    let args = Args::parse();
    if let Err(err) = run(args) {
        error!("{err}");
        exit(-1);
    }
}

fn init_tracing() {
    let fmt_layer = tracing_subscriber::fmt::layer().with_writer(stderr);
    let filter_layer = EnvFilter::builder()
        .with_env_var("PROJECTCTL_LOG_FILTER")
        .with_default_directive(Level::WARN.into())
        .from_env_lossy();
    let res = tracing_subscriber::registry()
        .with(fmt_layer)
        .with(filter_layer)
        .try_init();
    if let Err(err) = res {
        eprintln!("failed to initialize tracing: {err}");
    }
}

fn run(args: Args) -> ProjectctlResult {
    let fs = DefaultFileSystem::init(args.projectctl_dir, args.project_dir)?;
    let runner = CommandRunner::init(fs)?;
    runner.run(args.cmd)
}

mod cmd;
mod digest;
mod fs;
mod model;
mod render;
