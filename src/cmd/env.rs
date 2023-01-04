use std::{
    fmt::{self, Debug, Formatter},
    io::Write,
};

use log::{error, info};
use regex::Regex;

use crate::{
    cfg::{ConfigLoader, DefaultConfigLoader, EnvVarKind},
    cli::EnvCommandArguments,
    err::{Error, ErrorKind, Result},
    paths::{DefaultPaths, Paths},
    script::{DefaultScriptRunner, ScriptRunner},
};

const BASH_SHELL: &str = "/bin/bash";
const SPECIAL_CHARS_PATTERN: &str = "[\"$]";

pub struct EnvCommand {
    args: EnvCommandArguments,
    cfg_loader: Box<dyn ConfigLoader>,
    paths: Box<dyn Paths>,
    runner: Box<dyn ScriptRunner>,
}

impl EnvCommand {
    pub fn new(args: EnvCommandArguments) -> Self {
        Self {
            args,
            cfg_loader: Box::new(DefaultConfigLoader::new()),
            paths: Box::new(DefaultPaths::new()),
            runner: Box::new(DefaultScriptRunner),
        }
    }

    pub fn run(self, out: &mut dyn Write) -> Result<()> {
        info!("Computing value of environment variables");
        let cfg_filepaths = self
            .paths
            .config(self.args.cfg_filepaths, self.args.project_dirpath)?;
        let cfg = self.cfg_loader.load(&cfg_filepaths)?;
        for (key, kind) in cfg.env {
            let res = match kind {
                EnvVarKind::Literal(val) => Self::write_export_statement(&key, &val, out),
                EnvVarKind::Run(cmd) => self
                    .runner
                    .run(BASH_SHELL, &cmd)
                    .and_then(|stdout| Self::write_export_statement(&key, &stdout, out)),
            };
            if let Err(err) = res {
                error!("Unable to compute value of `{}`: {}", key, err);
                if let ErrorKind::ScriptFailed { rc, stderr, stdout } = err.kind {
                    if let Some(rc) = rc {
                        error!("Return code: {}", rc);
                    }
                    error!("stdout:\n{}", stdout);
                    error!("stderr:\n{}", stderr);
                }
            }
        }
        Ok(())
    }

    #[inline]
    fn write_export_statement(key: &str, val: &str, out: &mut dyn Write) -> Result<()> {
        let escape_regex = Regex::new(SPECIAL_CHARS_PATTERN).unwrap();
        let val = escape_regex.replace_all(val, "\\$0");
        writeln!(out, "export {}=\"{}\"", key, val).map_err(|err| Error {
            kind: ErrorKind::IO(err),
            msg: "Unable to write statement".into(),
        })
    }
}

impl Debug for EnvCommand {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        f.debug_struct("EnvCommand")
            .field("args", &self.args)
            .finish()
    }
}

#[cfg(test)]
mod test {
    use std::collections::HashMap;

    use tempfile::tempdir;

    use crate::{
        cfg::{Config, StubConfigLoader},
        paths::StubPaths,
        script::StubScriptRunner,
    };

    use super::*;

    mod env_command {
        use super::*;

        mod new {
            use super::*;

            #[test]
            fn cmd() {
                let args = EnvCommandArguments::default();
                let cmd = EnvCommand::new(args.clone());
                assert_eq!(cmd.args, args);
            }
        }

        mod run {
            use std::io;

            use super::*;

            struct Context {
                lit_key: &'static str,
                lit_val: &'static str,
                run_ko_key: &'static str,
                run_ko_script: &'static str,
                run_ok_key: &'static str,
                run_ok_script: &'static str,
                run_ok_val: &'static str,
            }

            struct Parameters {
                args: EnvCommandArguments,
                env: HashMap<String, EnvVarKind>,
            }

            #[test]
            fn ok_when_default_args() {
                test(
                    |_| Parameters {
                        args: EnvCommandArguments::default(),
                        env: HashMap::new(),
                    },
                    |_, res| {
                        let out = res.unwrap();
                        assert!(out.is_empty());
                    },
                )
            }

            #[test]
            fn ok_when_custom_args() {
                let project_dirpath = tempdir().unwrap().into_path();
                let cfg_filepath = tempdir().unwrap().into_path().join("cfg1");
                test(
                    |ctx| Parameters {
                        args: EnvCommandArguments {
                            cfg_filepaths: vec![cfg_filepath.clone()],
                            project_dirpath: Some(project_dirpath.clone()),
                        },
                        env: HashMap::from_iter([
                            (ctx.lit_key.into(), EnvVarKind::Literal(ctx.lit_val.into())),
                            (
                                ctx.run_ok_key.into(),
                                EnvVarKind::Run(ctx.run_ok_script.into()),
                            ),
                            (
                                ctx.run_ko_key.into(),
                                EnvVarKind::Run(ctx.run_ko_script.into()),
                            ),
                        ]),
                    },
                    |ctx, res| {
                        let out = res.unwrap();
                        let exports: Vec<&str> = out.lines().collect();
                        assert_export(ctx.lit_key, "\\\"\\$LIT", &exports);
                        assert_export(ctx.run_ok_key, "\\\"\\$RUN", &exports);
                    },
                )
            }

            fn assert_export(key: &str, val: &str, exports: &[&str]) {
                let expected_export = format!("export {}=\"{}\"", key, val);
                assert!(
                    exports.iter().any(|export| *export == expected_export),
                    "missing `{}`",
                    expected_export
                );
            }

            fn test<P: Fn(&Context) -> Parameters, A: Fn(&Context, Result<String>)>(
                create_params_fn: P,
                assert_fn: A,
            ) {
                let ctx = Context {
                    lit_key: "LIT",
                    lit_val: "\"$LIT",
                    run_ko_key: "RUN_KO",
                    run_ko_script: "echo KO",
                    run_ok_key: "RUN_OK",
                    run_ok_script: "echo OK",
                    run_ok_val: "\"$RUN",
                };
                let params = create_params_fn(&ctx);
                let cfg_loader = StubConfigLoader::new().with_stub_of_load({
                    let cfg_filepaths = params.args.cfg_filepaths.clone();
                    move |_, filepaths| {
                        assert_eq!(filepaths, cfg_filepaths);
                        Ok(Config {
                            env: params.env.clone(),
                        })
                    }
                });
                let paths = StubPaths::new().with_stub_of_config({
                    let expected_cfg_filepaths = params.args.cfg_filepaths.clone();
                    let expected_project_dirpath = params.args.project_dirpath.clone();
                    move |_, cfg_filepaths, project_dirpath| {
                        assert_eq!(cfg_filepaths, expected_cfg_filepaths);
                        assert_eq!(project_dirpath, expected_project_dirpath);
                        Ok(cfg_filepaths)
                    }
                });
                let runner = StubScriptRunner::new().with_stub_of_run({
                    move |_, shell, script| {
                        assert_eq!(shell, BASH_SHELL);
                        if script == ctx.run_ok_script {
                            Ok(ctx.run_ok_val.into())
                        } else if script == ctx.run_ko_script {
                            Err(Error {
                                kind: ErrorKind::IO(io::Error::from(
                                    io::ErrorKind::PermissionDenied,
                                )),
                                msg: "error".into(),
                            })
                        } else {
                            panic!("unexpected call of run");
                        }
                    }
                });
                let cmd = EnvCommand {
                    args: params.args,
                    cfg_loader: Box::new(cfg_loader),
                    paths: Box::new(paths),
                    runner: Box::new(runner),
                };
                let mut out = vec![];
                let res = cmd.run(&mut out).map(|_| String::from_utf8(out).unwrap());
                assert_fn(&ctx, res);
            }
        }
    }
}
