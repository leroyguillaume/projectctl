use std::{
    fmt::{self, Debug, Formatter},
    io::Write,
};

use liquid::{
    model::{KString, ScalarCow, Value},
    Object, ParserBuilder,
};
use log::trace;

use crate::{
    cli::{HookCommandArguments, HookCommandShellArgument, ENV_COMMAND},
    err::{Error, ErrorKind, Result},
    paths::{DefaultPaths, Paths},
    sys::{DefaultSystem, System},
};

const PROGRAM_NAME: &str = env!("CARGO_PKG_NAME");

const BASH_HOOK_TEMPLATE: &str = include_str!("../../resources/main/hooks/bash");
const ZSH_HOOK_TEMPLATE: &str = include_str!("../../resources/main/hooks/zsh");

const SHELL_ENV_VAR_KEY: &str = "SHELL";

const ALLOWED_DIRS_FILEPATH_VAR_KEY: &str = "allowed_dirs_filepath";
const ENV_COMMAND_VAR_KEY: &str = "env_cmd";
const PROGRAM_VAR_KEY: &str = "program";

pub struct HookCommand {
    args: HookCommandArguments,
    paths: Box<dyn Paths>,
    sys: Box<dyn System>,
}

impl HookCommand {
    pub fn new(args: HookCommandArguments) -> Self {
        Self {
            args,
            paths: Box::new(DefaultPaths::new()),
            sys: Box::new(DefaultSystem),
        }
    }

    pub fn run(self, out: &mut dyn Write) -> Result<()> {
        let shell = self
            .args
            .shell
            .map(|shell| match shell {
                HookCommandShellArgument::Bash => Ok(BASH_HOOK_TEMPLATE),
                HookCommandShellArgument::Zsh => Ok(ZSH_HOOK_TEMPLATE),
            })
            .unwrap_or_else(|| {
                if let Some(shell) = self.sys.env_var(SHELL_ENV_VAR_KEY) {
                    trace!(
                        "Environment variable {} has value `{}`",
                        SHELL_ENV_VAR_KEY,
                        shell
                    );
                    if shell.ends_with("bash") {
                        Ok(BASH_HOOK_TEMPLATE)
                    } else if shell.ends_with("zsh") {
                        Ok(ZSH_HOOK_TEMPLATE)
                    } else {
                        Err(Error {
                            kind: ErrorKind::UnsupportedShell,
                            msg: format!("Shell `{}` is not supported yet", shell),
                        })
                    }
                } else {
                    trace!("Environment variable {} is not defined", SHELL_ENV_VAR_KEY);
                    Err(Error {
                        kind: ErrorKind::UnsupportedShell,
                        msg: "Unable to retrieve current shell, please explicit it in command line"
                            .into(),
                    })
                }
            })?;
        let allowed_dirs_filepath = self
            .paths
            .allowed_dirs(self.args.allowed_dirs_filepath, None)?;
        let allowed_dirs_filepath = allowed_dirs_filepath.to_string_lossy().to_string();
        let parser = ParserBuilder::with_stdlib().build().unwrap();
        let tpl = parser.parse(shell).unwrap();
        let mut obj = Object::new();
        obj.insert(
            KString::from_static(ALLOWED_DIRS_FILEPATH_VAR_KEY),
            Value::Scalar(ScalarCow::from(allowed_dirs_filepath)),
        );
        obj.insert(
            KString::from_static(ENV_COMMAND_VAR_KEY),
            Value::Scalar(ScalarCow::from(ENV_COMMAND)),
        );
        obj.insert(
            KString::from_static(PROGRAM_VAR_KEY),
            Value::Scalar(ScalarCow::from(PROGRAM_NAME)),
        );
        tpl.render_to(out, &obj).unwrap();
        Ok(())
    }
}

impl Debug for HookCommand {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        f.debug_struct("HookBashCommand")
            .field("args", &self.args)
            .finish()
    }
}

#[cfg(test)]
mod test {
    use std::path::PathBuf;

    use tempfile::tempdir;

    use crate::{paths::StubPaths, sys::StubSystem};

    use super::*;

    mod hook_command {
        use super::*;

        mod new {
            use super::*;

            #[test]
            fn cmd() {
                let args = HookCommandArguments::default();
                let cmd = HookCommand::new(args.clone());
                assert_eq!(cmd.args, args);
            }
        }

        mod run {
            use super::*;

            struct Context {
                allowed_dirs_filepath: PathBuf,
            }

            struct Parameters {
                args: HookCommandArguments,
                shell_env_var_val: Option<&'static str>,
            }

            #[test]
            fn err_when_shell_env_var_retrieving_failed() {
                test(
                    |_| Parameters {
                        args: HookCommandArguments::default(),
                        shell_env_var_val: None,
                    },
                    |_, res| match res.unwrap_err().kind {
                        ErrorKind::UnsupportedShell => (),
                        kind => panic!("expected UnsupportedShell (actual: {:?})", kind),
                    },
                )
            }

            #[test]
            fn err_when_shell_is_unsupported() {
                let shell = "fish";
                test(
                    |_| Parameters {
                        args: HookCommandArguments::default(),
                        shell_env_var_val: Some(shell),
                    },
                    |_, res| match res.unwrap_err().kind {
                        ErrorKind::UnsupportedShell => (),
                        kind => panic!("expected UnsupportedShell (actual: {:?})", kind),
                    },
                )
            }

            #[test]
            fn ok_when_bash_auto_detect() {
                test(
                    |_| Parameters {
                        args: HookCommandArguments::default(),
                        shell_env_var_val: Some("/bin/bash"),
                    },
                    |ctx, res| assert(ctx, res, BASH_HOOK_TEMPLATE),
                )
            }

            #[test]
            fn ok_when_bash_arg() {
                test(
                    |_| Parameters {
                        args: HookCommandArguments {
                            allowed_dirs_filepath: Some("/dirs".into()),
                            shell: Some(HookCommandShellArgument::Bash),
                        },
                        shell_env_var_val: Some("/bin/zsh"),
                    },
                    |ctx, res| assert(ctx, res, BASH_HOOK_TEMPLATE),
                )
            }

            #[test]
            fn ok_when_zsh_auto_detect() {
                test(
                    |_| Parameters {
                        args: HookCommandArguments::default(),
                        shell_env_var_val: Some("/bin/zsh"),
                    },
                    |ctx, res| assert(ctx, res, ZSH_HOOK_TEMPLATE),
                )
            }

            #[test]
            fn ok_when_zsh_arg() {
                test(
                    |_| Parameters {
                        args: HookCommandArguments {
                            allowed_dirs_filepath: Some("/dirs".into()),
                            shell: Some(HookCommandShellArgument::Zsh),
                        },
                        shell_env_var_val: Some("/bin/bash"),
                    },
                    |ctx, res| assert(ctx, res, ZSH_HOOK_TEMPLATE),
                )
            }

            fn assert(ctx: &Context, res: Result<String>, tpl: &str) {
                let out = res.unwrap();
                let parser = ParserBuilder::with_stdlib().build().unwrap();
                let tpl = parser.parse(tpl).unwrap();
                let mut obj = Object::new();
                obj.insert(
                    KString::from_static(ALLOWED_DIRS_FILEPATH_VAR_KEY),
                    Value::Scalar(ScalarCow::from(
                        ctx.allowed_dirs_filepath.to_string_lossy().to_string(),
                    )),
                );
                obj.insert(
                    KString::from_static(ENV_COMMAND_VAR_KEY),
                    Value::Scalar(ScalarCow::from(ENV_COMMAND)),
                );
                obj.insert(
                    KString::from_static(PROGRAM_VAR_KEY),
                    Value::Scalar(ScalarCow::from(PROGRAM_NAME)),
                );
                let mut expected_out = vec![];
                tpl.render_to(&mut expected_out, &obj).unwrap();
                let expected_out = String::from_utf8(expected_out).unwrap();
                assert_eq!(out, expected_out);
            }

            fn test<P: Fn(&Context) -> Parameters, A: Fn(&Context, Result<String>)>(
                create_params_fn: P,
                assert_fn: A,
            ) {
                let ctx = Context {
                    allowed_dirs_filepath: tempdir().unwrap().into_path().join("allowed-dirs"),
                };
                let params = create_params_fn(&ctx);
                let paths = StubPaths::new().with_stub_of_allowed_dirs({
                    let filepath = ctx.allowed_dirs_filepath.clone();
                    let expected_allowed_dirs_filepaths = params.args.allowed_dirs_filepath.clone();
                    move |_, allowed_dirs_filepath, cfg_filepath| {
                        assert_eq!(allowed_dirs_filepath, expected_allowed_dirs_filepaths);
                        assert!(cfg_filepath.is_none());
                        Ok(filepath.clone())
                    }
                });
                let sys = StubSystem::new().with_stub_of_env_var(move |_, key| {
                    assert_eq!(key, SHELL_ENV_VAR_KEY);
                    params.shell_env_var_val.map(|shell| shell.into())
                });
                let cmd = HookCommand {
                    args: params.args,
                    paths: Box::new(paths),
                    sys: Box::new(sys),
                };
                let mut out = vec![];
                let res = cmd.run(&mut out).map(|_| String::from_utf8(out).unwrap());
                assert_fn(&ctx, res);
            }
        }
    }
}
