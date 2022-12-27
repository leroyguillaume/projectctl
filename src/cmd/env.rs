use std::{
    fmt::{self, Debug, Formatter},
    io::Write,
};

use regex::Regex;

use crate::{
    cfg::{ConfigLoader, DefaultConfigLoader, EnvVarKind},
    cli::EnvCommandArguments,
    err::{Error, Result},
    paths::{DefaultPaths, Paths},
};

const SPECIAL_CHARS_PATTERN: &str = "[\"$]";

pub struct EnvCommand {
    args: EnvCommandArguments,
    cfg_loader: Box<dyn ConfigLoader>,
    paths: Box<dyn Paths>,
}

impl EnvCommand {
    pub fn new(args: EnvCommandArguments) -> Self {
        Self {
            args,
            cfg_loader: Box::new(DefaultConfigLoader::new()),
            paths: Box::new(DefaultPaths::new()),
        }
    }

    pub fn run(self, out: &mut dyn Write) -> Result<()> {
        let cfg_filepaths = self
            .paths
            .config(self.args.cfg_filepaths, self.args.project_dirpath)?;
        let cfg = self.cfg_loader.load(&cfg_filepaths)?;
        let val_regex = Regex::new(SPECIAL_CHARS_PATTERN).unwrap();
        for (key, kind) in cfg.env {
            match kind {
                EnvVarKind::Literal(val) => {
                    let val = val_regex.replace_all(&val, "\\$0");
                    writeln!(out, "export {}=\"{}\"", key, val).map_err(Error::IO)?
                }
            }
        }
        Ok(())
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
            use super::*;

            struct Parameters {
                args: EnvCommandArguments,
                env: HashMap<String, EnvVarKind>,
            }

            #[test]
            fn ok_when_default_args() {
                test(
                    || Parameters {
                        args: EnvCommandArguments::default(),
                        env: HashMap::new(),
                    },
                    |res| {
                        let out = res.unwrap();
                        assert!(out.is_empty());
                    },
                )
            }

            #[test]
            fn ok_when_custom_args() {
                let var_key = "VAR";
                let project_dirpath = tempdir().unwrap().into_path();
                let cfg_filepath = tempdir().unwrap().into_path().join("cfg1");
                test(
                    || Parameters {
                        args: EnvCommandArguments {
                            cfg_filepaths: vec![cfg_filepath.clone()],
                            project_dirpath: Some(project_dirpath.clone()),
                        },
                        env: HashMap::from_iter([(
                            var_key.into(),
                            EnvVarKind::Literal("\"$VAL".into()),
                        )]),
                    },
                    |res| assert_export(res, var_key, "\\\"\\$VAL"),
                )
            }

            fn assert_export(res: Result<String>, var_key: &str, var_val: &str) {
                let out = res.unwrap();
                let expected_out = format!("export {}=\"{}\"\n", var_key, var_val);
                assert_eq!(out, expected_out);
            }

            fn test<P: Fn() -> Parameters, A: Fn(Result<String>)>(
                create_params_fn: P,
                assert_fn: A,
            ) {
                let params = create_params_fn();
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
                let cmd = EnvCommand {
                    args: params.args,
                    cfg_loader: Box::new(cfg_loader),
                    paths: Box::new(paths),
                };
                let mut out = vec![];
                let res = cmd.run(&mut out).map(|_| String::from_utf8(out).unwrap());
                assert_fn(res);
            }
        }
    }
}
