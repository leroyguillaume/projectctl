use std::{
    error::Error,
    fmt::{self, Display, Formatter},
    path::PathBuf,
};

use clap::{ArgAction, Args, Parser, Subcommand};
use log::LevelFilter;

use crate::cmd::{new::NewCommand, CommandKind};

pub const DEFAULT_TPL_GIT_REPO_URL: &str = "https://github.com/leroyguillaume/projectctl-templates";

#[derive(Debug, Parser)]
#[command(author, version, about, long_about = None)]
pub struct Arguments {
    #[command(subcommand)]
    pub cmd: CommandArgument,

    #[command(flatten)]
    pub logging: LoggingArguments,
}

impl Arguments {
    pub fn into_command_kind(self) -> CommandKind {
        match self.cmd {
            CommandArgument::New(args) => CommandKind::New(NewCommand::new(args)),
        }
    }
}

#[derive(Args, Clone, Debug, Default, Eq, PartialEq)]
pub struct LoggingArguments {
    #[arg(
        action = ArgAction::Count,
        conflicts_with = "verbose",
        help = "Less logs per occurence",
        long,
        short = 'q',
    )]
    pub quiet: u8,

    #[clap(help = "Disable colors in logs", long)]
    pub no_color: bool,

    #[clap(
        action = ArgAction::Count,
        help = "More logs per occurence",
        long,
        short = 'v',
    )]
    pub verbose: u8,
}

impl LoggingArguments {
    pub fn to_level_filter(&self) -> LevelFilter {
        if self.quiet > 0 {
            match self.quiet {
                1 => LevelFilter::Error,
                _ => LevelFilter::Off,
            }
        } else if self.verbose > 0 {
            match self.verbose {
                1 => LevelFilter::Info,
                2 => LevelFilter::Debug,
                _ => LevelFilter::Trace,
            }
        } else {
            LevelFilter::Warn
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
    #[clap(help = "Destination directory", index = 3, name = "DIR")]
    pub dest: Option<PathBuf>,

    #[clap(
        default_value = DEFAULT_TPL_GIT_REPO_URL,
        help = "URL to git repository that contains templates",
        long,
        name = "URL"
    )]
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

    #[clap(help = "Name of the project to create", index = 2)]
    pub name: String,

    #[clap(help = "Name of the template to use", index = 1, name = "TEMPLATE")]
    pub tpl: String,

    #[clap(
        help = "Define custom variable",
        long = "set",
        name = "KEY=VALUE",
        number_of_values = 1,
        short = 'd',
        value_parser = parse_key_val,
    )]
    pub vars: Vec<(String, String)>,
}

#[cfg(test)]
impl NewCommandArguments {
    pub fn default_for_test() -> Self {
        Self {
            dest: None,
            git: DEFAULT_TPL_GIT_REPO_URL.into(),
            git_branch: None,
            git_tag: None,
            name: String::from("myproject"),
            tpl: String::from("mytemplate"),
            vars: vec![("myvar".into(), "myvalue".into())],
        }
    }
}

#[derive(Debug)]
struct InvalidVariableError(String);

impl Error for InvalidVariableError {}

impl Display for InvalidVariableError {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        write!(f, "`{}` does not contain `=`", self.0)
    }
}

fn parse_key_val(s: &str) -> Result<(String, String), InvalidVariableError> {
    if let Some((var, value)) = s.split_once('=') {
        Ok((var.into(), value.into()))
    } else {
        Err(InvalidVariableError(s.into()))
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
                            logging: LoggingArguments::default(),
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

    mod logging_arguments {
        use super::*;

        mod to_level_filter {
            use super::*;

            macro_rules! test {
                ($ident:ident, $quiet:literal, $verbose:literal, $lvl:expr) => {
                    #[test]
                    fn $ident() {
                        let args = LoggingArguments {
                            quiet: $quiet,
                            no_color: false,
                            verbose: $verbose,
                        };
                        assert_eq!(args.to_level_filter(), $lvl);
                    }
                };
            }

            test!(off, 2, 0, LevelFilter::Off);
            test!(error, 1, 0, LevelFilter::Error);
            test!(warn, 0, 0, LevelFilter::Warn);
            test!(info, 0, 1, LevelFilter::Info);
            test!(debug, 0, 2, LevelFilter::Debug);
            test!(trace, 0, 3, LevelFilter::Trace);
        }
    }
}
