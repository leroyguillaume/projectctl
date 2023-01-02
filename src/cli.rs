use std::path::PathBuf;

use clap::{ArgAction, Args, Parser, Subcommand, ValueEnum};
use log::LevelFilter;
use serde_json::{Map, Value};

use crate::{
    cmd::{
        destroy::DestroyCommand, env::EnvCommand, hook::HookCommand, new::NewCommand, CommandKind,
    },
    err::{Error, Result},
};

pub type Values = Map<String, Value>;

pub const ENV_COMMAND: &str = "env";

const ALLOWED_DIRS_OPT_HELP: &str = "Path to file that contains allowed directories list";
const ALLOWED_DIRS_OPT_LONG: &str = "allowed-dirs";
const ALLOWED_DIRS_OPT_NAME: &str = "ALLOWED DIRS FILE";
const PROJECT_DIR_OPT_HELP: &str = "Path to project directory";
const PROJECT_DIR_OPT_NAME: &str = "PROJECT DIR";

const DEFAULT_TPL_GIT_REPO_URL: &str = "https://github.com/leroyguillaume/projectctl-templates";

#[derive(Debug, Parser)]
#[command(author, version = env!("VERSION"), about, long_about = None)]
pub struct Arguments {
    #[command(subcommand)]
    pub cmd: CommandArgument,

    #[command(flatten)]
    pub logging: LoggingArguments,
}

impl Arguments {
    pub fn into_command_kind(self) -> CommandKind {
        match self.cmd {
            CommandArgument::Destroy(args) => {
                CommandKind::Destroy(Box::new(DestroyCommand::new(args)))
            }
            CommandArgument::Env(args) => CommandKind::Env(Box::new(EnvCommand::new(args))),
            CommandArgument::Hook(args) => CommandKind::Hook(Box::new(HookCommand::new(args))),
            CommandArgument::New(args) => CommandKind::New(Box::new(NewCommand::new(args))),
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
    #[command(about = "Delete a project")]
    Destroy(DestroyCommandArguments),

    #[command(about = "Print environment", name = ENV_COMMAND)]
    Env(EnvCommandArguments),

    #[command(about = "Print shell hook")]
    Hook(HookCommandArguments),

    #[command(about = "Create a new project from template")]
    New(NewCommandArguments),
}

#[derive(Args, Clone, Debug, Eq, PartialEq)]
pub struct DestroyCommandArguments {
    #[clap(
        help = ALLOWED_DIRS_OPT_HELP,
        long = ALLOWED_DIRS_OPT_LONG,
        name = ALLOWED_DIRS_OPT_NAME
    )]
    pub allowed_dirs_filepath: Option<PathBuf>,

    #[clap(
        help = PROJECT_DIR_OPT_HELP,
        name = PROJECT_DIR_OPT_NAME,
    )]
    pub project_dirpath: PathBuf,
}

#[cfg(test)]
impl DestroyCommandArguments {
    pub fn new(project_dirpath: PathBuf) -> Self {
        Self {
            allowed_dirs_filepath: None,
            project_dirpath,
        }
    }
}

#[derive(Args, Clone, Debug, Default, Eq, PartialEq)]
pub struct EnvCommandArguments {
    #[clap(
        help = "Configuration files (from least to most priority)",
        long = "config",
        name = "CONFIG FILE",
        short = 'c'
    )]
    pub cfg_filepaths: Vec<PathBuf>,

    #[clap(
        help = PROJECT_DIR_OPT_HELP,
        long = "project",
        name = PROJECT_DIR_OPT_NAME,
        short = 'p'
    )]
    pub project_dirpath: Option<PathBuf>,
}

#[derive(Args, Clone, Debug, Default, Eq, PartialEq)]
pub struct HookCommandArguments {
    #[clap(
        help = ALLOWED_DIRS_OPT_HELP,
        long = ALLOWED_DIRS_OPT_LONG,
        name = ALLOWED_DIRS_OPT_NAME
    )]
    pub allowed_dirs_filepath: Option<PathBuf>,

    #[clap()]
    pub shell: Option<HookCommandShellArgument>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, ValueEnum)]
pub enum HookCommandShellArgument {
    Bash,
    Zsh,
}

#[derive(Args, Clone, Debug, Eq, PartialEq)]
pub struct NewCommandArguments {
    #[clap(
        help = ALLOWED_DIRS_OPT_HELP,
        long = ALLOWED_DIRS_OPT_LONG,
        name = ALLOWED_DIRS_OPT_NAME
    )]
    pub allowed_dirs_filepath: Option<PathBuf>,

    #[clap(
        help = "Description of the project to create",
        long = "description",
        name = "DESCRIPTION",
        short = 'd'
    )]
    pub desc: Option<String>,

    #[clap(help = "Destination directory", index = 3, name = "DIR")]
    pub dest: Option<PathBuf>,

    #[clap(help = "Name of the project to create", index = 2)]
    pub name: String,

    #[clap(
        help = "Don't create configuration examples if they don't exist",
        long = "skip-config-examples"
    )]
    pub skip_config_examples: bool,

    #[clap(
        help = "Don't update gitignore if it doesn't contain projectctl files",
        long = "skip-gitignore"
    )]
    pub skip_gitignore_update: bool,

    #[clap(help = "Name of the template to use", index = 1, name = "TEMPLATE")]
    pub tpl: String,

    #[clap(
        help = "Name of the branch of the git repository to checkout",
        long = "git-branch",
        name = "BRANCH"
    )]
    pub tpl_repo_branch: Option<String>,

    #[clap(
        conflicts_with = "BRANCH",
        help = "Name of the tag of the git repository to checkout",
        long = "git-tag",
        name = "TAG"
    )]
    pub tpl_repo_tag: Option<String>,

    #[clap(
        default_value = DEFAULT_TPL_GIT_REPO_URL,
        help = "URL to git repository that contains templates",
        long = "git",
        name = "URL"
    )]
    pub tpl_repo_url: String,

    #[clap(help = "Define custom variables (JSON format)", long, value_parser = parse_values)]
    pub values: Option<Values>,
}

#[cfg(test)]
impl NewCommandArguments {
    pub fn new(tpl: String, name: String) -> Self {
        Self {
            allowed_dirs_filepath: None,
            desc: None,
            dest: None,
            name,
            skip_config_examples: false,
            skip_gitignore_update: false,
            tpl,
            tpl_repo_url: DEFAULT_TPL_GIT_REPO_URL.into(),
            tpl_repo_branch: None,
            tpl_repo_tag: None,
            values: None,
        }
    }
}

fn parse_values(json: &str) -> Result<Values> {
    match serde_json::from_str(json).map_err(Error::MalformedValues)? {
        Value::Object(json) => Ok(json),
        _ => Err(Error::NotAJsonObject),
    }
}

#[cfg(test)]
mod test {
    use serde_json::json;

    use super::*;

    mod arguments {
        use super::*;

        mod into_command_kind {
            use super::*;

            struct Parameters {
                cmd: CommandArgument,
            }

            #[test]
            fn destroy() {
                test(
                    || Parameters {
                        cmd: CommandArgument::Destroy(DestroyCommandArguments::new(PathBuf::from(
                            "my-project",
                        ))),
                    },
                    |kind| match kind {
                        CommandKind::Destroy(_) => (),
                        kind => panic!("expected Destroy (actual: {:?})", kind),
                    },
                )
            }

            #[test]
            fn env() {
                test(
                    || Parameters {
                        cmd: CommandArgument::Env(EnvCommandArguments::default()),
                    },
                    |kind| match kind {
                        CommandKind::Env(_) => (),
                        kind => panic!("expected Env (actual: {:?})", kind),
                    },
                )
            }

            #[test]
            fn hook() {
                test(
                    || Parameters {
                        cmd: CommandArgument::Hook(HookCommandArguments::default()),
                    },
                    |kind| match kind {
                        CommandKind::Hook(_) => (),
                        kind => panic!("expected Hook (actual: {:?})", kind),
                    },
                )
            }

            #[test]
            fn new() {
                test(
                    || Parameters {
                        cmd: CommandArgument::New(NewCommandArguments::new(
                            "test".into(),
                            "my-project".into(),
                        )),
                    },
                    |kind| match kind {
                        CommandKind::New(_) => (),
                        kind => panic!("expected New (actual: {:?})", kind),
                    },
                )
            }

            fn test<P: Fn() -> Parameters, A: Fn(CommandKind)>(create_params_fn: P, assert_fn: A) {
                let params = create_params_fn();
                let args = Arguments {
                    cmd: params.cmd,
                    logging: LoggingArguments::default(),
                };
                let kind = args.into_command_kind();
                assert_fn(kind);
            }
        }
    }

    mod logging_arguments {
        use super::*;

        mod to_level_filter {
            use super::*;

            struct Parameters {
                quiet: u8,
                verbose: u8,
            }

            #[test]
            fn debug() {
                test(
                    || Parameters {
                        quiet: 0,
                        verbose: 2,
                    },
                    |filter| assert_eq!(filter, LevelFilter::Debug),
                );
            }

            #[test]
            fn error() {
                test(
                    || Parameters {
                        quiet: 1,
                        verbose: 0,
                    },
                    |filter| assert_eq!(filter, LevelFilter::Error),
                );
            }

            #[test]
            fn info() {
                test(
                    || Parameters {
                        quiet: 0,
                        verbose: 1,
                    },
                    |filter| assert_eq!(filter, LevelFilter::Info),
                );
            }

            #[test]
            fn off() {
                test(
                    || Parameters {
                        quiet: 2,
                        verbose: 0,
                    },
                    |filter| assert_eq!(filter, LevelFilter::Off),
                );
            }

            #[test]
            fn trace() {
                test(
                    || Parameters {
                        quiet: 0,
                        verbose: 3,
                    },
                    |filter| assert_eq!(filter, LevelFilter::Trace),
                );
            }

            #[test]
            fn warn() {
                test(
                    || Parameters {
                        quiet: 0,
                        verbose: 0,
                    },
                    |filter| assert_eq!(filter, LevelFilter::Warn),
                );
            }

            fn test<P: Fn() -> Parameters, A: Fn(LevelFilter)>(create_params_fn: P, assert_fn: A) {
                let params = create_params_fn();
                let args = LoggingArguments {
                    quiet: params.quiet,
                    verbose: params.verbose,
                    ..LoggingArguments::default()
                };
                let filter = args.to_level_filter();
                assert_fn(filter);
            }
        }
    }

    mod parse_values {
        use super::*;

        struct Parameters {
            arg: String,
        }

        #[test]
        fn err_when_json_is_not_obj() {
            test(
                || Parameters {
                    arg: "\"test\"".into(),
                },
                |res| match res.unwrap_err() {
                    Error::NotAJsonObject => (),
                    err => panic!("expected NotAJsonObject (actual: {:?})", err),
                },
            )
        }

        #[test]
        fn ok_when_json_is_obj() {
            let key = "test";
            let val = 3;
            test(
                || Parameters {
                    arg: format!("{{\"{}\":{}}}", key, val),
                },
                |res| {
                    let expected_values = json!({ key: val });
                    let values = Value::Object(res.unwrap());
                    assert_eq!(values, expected_values);
                },
            )
        }

        fn test<P: Fn() -> Parameters, A: Fn(Result<Values>)>(create_params_fn: P, assert_fn: A) {
            let params = create_params_fn();
            let res = parse_values(&params.arg);
            assert_fn(res);
        }
    }
}
