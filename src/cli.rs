use clap::Parser;
use clap_verbosity_flag::Verbosity;

#[derive(Debug, Parser)]
#[command(author, version, about, long_about = None)]
pub struct Arguments {
    #[command(flatten)]
    pub verbosity: Verbosity,
}
