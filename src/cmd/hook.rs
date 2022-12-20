use std::{
    env::{var, VarError},
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
    consts::PROGRAM_NAME,
    err::{Error, Result},
    fs::{DefaultFileSystem, FileSystem},
    utils::allowed_dirs_filepath,
};

const BASH_HOOK_TEMPLATE: &str = include_str!("../../resources/main/hooks/bash");
const ZSH_HOOK_TEMPLATE: &str = include_str!("../../resources/main/hooks/zsh");

const SHELL_ENV_VAR_KEY: &str = "SHELL";

type EnvFn = dyn Fn(&str) -> std::result::Result<String, VarError>;

pub struct HookCommand {
    args: HookCommandArguments,
    env_fn: Box<EnvFn>,
    fs: Box<dyn FileSystem>,
}

impl HookCommand {
    pub fn new(args: HookCommandArguments) -> Self {
        Self {
            args,
            env_fn: Box::new(|key| var(key)),
            fs: Box::new(DefaultFileSystem),
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
                trace!(
                    "Retrieving `{}` environment variable value",
                    SHELL_ENV_VAR_KEY
                );
                let shell = (self.env_fn)(SHELL_ENV_VAR_KEY).unwrap_or_default();
                trace!("`{}`: {}", SHELL_ENV_VAR_KEY, shell);
                if shell.ends_with("bash") {
                    Ok(BASH_HOOK_TEMPLATE)
                } else if shell.ends_with("zsh") {
                    Ok(ZSH_HOOK_TEMPLATE)
                } else {
                    Err(Error::UnsupportedShell(shell))
                }
            })?;
        let allowed_dirs_filepath =
            allowed_dirs_filepath(self.args.allowed_dirs_filepath, self.fs.as_ref())?;
        let allowed_dirs_filepath = allowed_dirs_filepath.to_string_lossy().to_string();
        let parser = ParserBuilder::with_stdlib().build().unwrap();
        let tpl = parser.parse(shell).unwrap();
        let mut obj = Object::new();
        obj.insert(
            KString::from_static("allowed_dirs_filepath"),
            Value::Scalar(ScalarCow::from(allowed_dirs_filepath)),
        );
        obj.insert(
            KString::from_static("env_cmd"),
            Value::Scalar(ScalarCow::from(ENV_COMMAND)),
        );
        obj.insert(
            KString::from_static("program"),
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
    use std::path::{Path, PathBuf};

    use tempfile::tempdir;

    use crate::{
        consts::{CONFIG_DIRNAME, DEFAULT_ALLOWED_DIRS_FILENAME},
        fs::StubFileSystem,
    };

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
                home_dirpath: PathBuf,
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
                    |_, res| match res.unwrap_err() {
                        Error::UnsupportedShell(shell) => assert!(shell.is_empty()),
                        err => panic!("expected UnsupportedShell (actual: {:?})", err),
                    },
                )
            }

            #[test]
            fn err_when_shell_is_unsupported() {
                let expected_shell = "fish";
                test(
                    |_| Parameters {
                        args: HookCommandArguments::default(),
                        shell_env_var_val: Some(expected_shell),
                    },
                    |_, res| match res.unwrap_err() {
                        Error::UnsupportedShell(shell) => assert_eq!(shell, expected_shell),
                        err => panic!("expected UnsupportedShell (actual: {:?})", err),
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
                    |ctx, res| {
                        assert_ok(
                            res,
                            &ctx.home_dirpath
                                .join(CONFIG_DIRNAME)
                                .join(DEFAULT_ALLOWED_DIRS_FILENAME),
                            BASH_HOOK_TEMPLATE,
                        )
                    },
                )
            }

            #[test]
            fn ok_when_bash_arg() {
                let allowed_dirs_filepath = Path::new("/dirs");
                test(
                    |_| Parameters {
                        args: HookCommandArguments {
                            allowed_dirs_filepath: Some(allowed_dirs_filepath.to_path_buf()),
                            shell: Some(HookCommandShellArgument::Bash),
                        },
                        shell_env_var_val: Some("/bin/zsh"),
                    },
                    |_, res| assert_ok(res, allowed_dirs_filepath, BASH_HOOK_TEMPLATE),
                )
            }

            #[test]
            fn ok_when_zsh_auto_detect() {
                test(
                    |_| Parameters {
                        args: HookCommandArguments::default(),
                        shell_env_var_val: Some("/bin/zsh"),
                    },
                    |ctx, res| {
                        assert_ok(
                            res,
                            &ctx.home_dirpath
                                .join(CONFIG_DIRNAME)
                                .join(DEFAULT_ALLOWED_DIRS_FILENAME),
                            ZSH_HOOK_TEMPLATE,
                        )
                    },
                )
            }

            #[test]
            fn ok_when_zsh_arg() {
                let allowed_dirs_filepath = Path::new("/dirs");
                test(
                    |_| Parameters {
                        args: HookCommandArguments {
                            allowed_dirs_filepath: Some(allowed_dirs_filepath.to_path_buf()),
                            shell: Some(HookCommandShellArgument::Zsh),
                        },
                        shell_env_var_val: Some("/bin/bash"),
                    },
                    |_, res| assert_ok(res, allowed_dirs_filepath, ZSH_HOOK_TEMPLATE),
                )
            }

            fn assert_ok(res: crate::err::Result<String>, allowed_dirs_filepath: &Path, tpl: &str) {
                let out = res.unwrap();
                let allowed_dirs_filepath = allowed_dirs_filepath.to_string_lossy().to_string();
                let parser = ParserBuilder::with_stdlib().build().unwrap();
                let tpl = parser.parse(tpl).unwrap();
                let mut obj = Object::new();
                obj.insert(
                    KString::from_static("allowed_dirs_filepath"),
                    Value::Scalar(ScalarCow::from(allowed_dirs_filepath)),
                );
                obj.insert(
                    KString::from_static("env_cmd"),
                    Value::Scalar(ScalarCow::from(ENV_COMMAND)),
                );
                obj.insert(
                    KString::from_static("program"),
                    Value::Scalar(ScalarCow::from(PROGRAM_NAME)),
                );
                let mut expected_out = vec![];
                tpl.render_to(&mut expected_out, &obj).unwrap();
                let expected_out = String::from_utf8(expected_out).unwrap();
                assert_eq!(out, expected_out);
            }

            fn test<P: Fn(&Context) -> Parameters, A: Fn(&Context, crate::err::Result<String>)>(
                create_params_fn: P,
                assert_fn: A,
            ) {
                let ctx = Context {
                    home_dirpath: tempdir().unwrap().into_path(),
                };
                let params = create_params_fn(&ctx);
                let fs = StubFileSystem::new().with_stub_of_home_dirpath({
                    let home_dirpath = ctx.home_dirpath.clone();
                    move |_| Ok(home_dirpath.clone())
                });
                let env_fn = move |key: &str| -> std::result::Result<String, VarError> {
                    assert_eq!(key, SHELL_ENV_VAR_KEY);
                    if let Some(shell) = params.shell_env_var_val {
                        Ok(shell.into())
                    } else {
                        Err(VarError::NotPresent)
                    }
                };
                let cmd = HookCommand {
                    args: params.args,
                    env_fn: Box::new(env_fn),
                    fs: Box::new(fs),
                };
                let mut out = vec![];
                let res = cmd.run(&mut out).map(|_| String::from_utf8(out).unwrap());
                assert_fn(&ctx, res);
            }
        }
    }
}
