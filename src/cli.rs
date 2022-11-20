use clap::{Args, Parser, Subcommand};
use clap_verbosity_flag::Verbosity;

const DEFAULT_TPL_GIT_REPO_URL: &str = "https://github.com/leroyguillaume/projectctl-templates";

#[derive(Debug, Parser)]
#[command(author, version, about, long_about = None)]
pub struct Arguments {
    #[command(subcommand)]
    pub cmd: CommandArgument,

    #[command(flatten)]
    pub verbosity: Verbosity,
}

#[derive(Debug, Subcommand)]
pub enum CommandArgument {
    #[command(about = "Create new project from template")]
    New(NewCommandArguments),
}

#[derive(Args, Debug)]
pub struct NewCommandArguments {
    #[clap(default_value = DEFAULT_TPL_GIT_REPO_URL, help = "URL to git repository that contains templates", long, name = "URL")]
    pub git: String,

    #[clap(
        help = "Name of the branch of the git repository to checkout",
        long,
        name = "BRANCH"
    )]
    pub git_branch: Option<String>,

    #[clap(
        conflicts_with = "BRANCH",
        help = "Name of the tag of the git repository to checkout",
        long,
        name = "TAG"
    )]
    pub git_tag: Option<String>,

    #[clap(help = "Name of the project to create")]
    pub name: String,
}
