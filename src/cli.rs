use std::path::PathBuf;

use clap::{Args, Parser, Subcommand};
use clap_verbosity_flag::Verbosity;

use crate::cmd::{new::NewCommand, CommandKind};

pub const DEFAULT_TPL_GIT_REPO_URL: &str = "https://github.com/leroyguillaume/projectctl-templates";

#[derive(Debug, Parser)]
#[command(author, version, about, long_about = None)]
pub struct Arguments {
    #[command(subcommand)]
    pub cmd: CommandArgument,

    #[command(flatten)]
    pub verbosity: Verbosity,
}

impl Arguments {
    pub fn into_command_kind(self) -> CommandKind {
        match self.cmd {
            CommandArgument::New(args) => CommandKind::New(NewCommand::new(args)),
        }
    }
}

#[derive(Debug, Subcommand)]
pub enum CommandArgument {
    #[command(about = "Create new project from template")]
    New(NewCommandArguments),
}

#[derive(Args, Clone, Debug, Eq, PartialEq)]
pub struct NewCommandArguments {
    #[clap(help = "Destination directory", long, name = "DIR")]
    pub dest: Option<PathBuf>,

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

#[cfg(test)]
impl NewCommandArguments {
    pub fn default_for_test() -> Self {
        Self {
            dest: None,
            git: DEFAULT_TPL_GIT_REPO_URL.into(),
            git_branch: None,
            git_tag: None,
            name: String::from("test"),
        }
    }
}

#[cfg(test)]
mod test {
    use super::*;

    mod arguments {
        use super::*;

        mod into_command_kind {
            use super::*;

            macro_rules! test {
                ($ident:ident, $cmd:expr, $kind:path) => {
                    #[test]
                    fn $ident() {
                        let args = Arguments {
                            cmd: $cmd,
                            verbosity: Verbosity::new(0, 0),
                        };
                        match args.into_command_kind() {
                            $kind(_) => (),
                        }
                    }
                };
            }

            test!(
                new,
                CommandArgument::New(NewCommandArguments::default_for_test()),
                CommandKind::New
            );
        }
    }
}
