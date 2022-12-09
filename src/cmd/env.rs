use std::{
    fmt::{self, Debug, Formatter},
    io::Write,
    path::Path,
};

use regex::Regex;

use crate::{
    cfg::{ConfigLoader, DefaultConfigLoader, EnvVarKind},
    cli::EnvCommandArguments,
    consts::{LOCAL_CONFIG_FILENAME, PROJECT_CONFIG_FILENAME},
    err::Error,
    fs::{DefaultFileSystem, FileSystem},
};

use super::Result;

const SPECIAL_CHARS_PATTERN: &str = "[\"$]";

pub struct EnvCommand {
    args: EnvCommandArguments,
    cfg_loader: Box<dyn ConfigLoader>,
    fs: Box<dyn FileSystem>,
}

impl EnvCommand {
    pub fn new(args: EnvCommandArguments) -> Self {
        Self {
            args,
            cfg_loader: Box::new(DefaultConfigLoader::new()),
            fs: Box::new(DefaultFileSystem),
        }
    }

    pub fn run(self, out: &mut dyn Write) -> Result {
        let project_dirpath = self
            .args
            .project_dirpath
            .map(Ok)
            .unwrap_or_else(|| self.fs.cwd())?;
        let default_cfg_filepath = project_dirpath.join(PROJECT_CONFIG_FILENAME);
        let local_cfg_filepath = project_dirpath.join(LOCAL_CONFIG_FILENAME);
        let cfg_filepaths = if self.args.cfg_filepaths.is_empty() {
            let mut cfg_filepaths = vec![];
            if default_cfg_filepath.exists() {
                cfg_filepaths.push(Path::new(&default_cfg_filepath));
            }
            if local_cfg_filepath.exists() {
                cfg_filepaths.push(Path::new(&local_cfg_filepath));
            }
            cfg_filepaths
        } else {
            self.args.cfg_filepaths.iter().map(Path::new).collect()
        };
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
    use std::{collections::HashMap, fs::File, path::PathBuf};

    use tempfile::tempdir;

    use crate::{
        cfg::{Config, StubConfigLoader},
        fs::StubFileSystem,
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

            struct Context {
                cwd: PathBuf,
            }

            struct Expected {
                cfg_filepaths: Vec<PathBuf>,
            }

            struct Parameters {
                args: EnvCommandArguments,
                env: HashMap<String, EnvVarKind>,
            }

            #[test]
            fn ok_when_no_cfg() {
                test(
                    |_| Parameters {
                        args: EnvCommandArguments::default(),
                        env: HashMap::new(),
                    },
                    |_| Expected {
                        cfg_filepaths: vec![],
                    },
                    |_, res| {
                        let out = res.unwrap();
                        assert!(out.is_empty());
                    },
                )
            }

            #[test]
            fn ok_when_default_cfg() {
                let var_key = "VAR";
                test(
                    |_| Parameters {
                        args: EnvCommandArguments::default(),
                        env: HashMap::from_iter([(
                            var_key.into(),
                            EnvVarKind::Literal("\"$VAL".into()),
                        )]),
                    },
                    |ctx| Expected {
                        cfg_filepaths: vec![
                            ctx.cwd.join(PROJECT_CONFIG_FILENAME),
                            ctx.cwd.join(LOCAL_CONFIG_FILENAME),
                        ],
                    },
                    |_, res| assert_export(res, var_key, "\\\"\\$VAL"),
                )
            }

            #[test]
            fn ok_when_project_dirpath_is_some() {
                let var_key = "VAR";
                let project_dirpath = tempdir().unwrap().into_path();
                test(
                    |_| Parameters {
                        args: EnvCommandArguments {
                            project_dirpath: Some(project_dirpath.clone()),
                            ..EnvCommandArguments::default()
                        },
                        env: HashMap::from_iter([(
                            var_key.into(),
                            EnvVarKind::Literal("\"$VAL".into()),
                        )]),
                    },
                    |_| Expected {
                        cfg_filepaths: vec![
                            project_dirpath.join(PROJECT_CONFIG_FILENAME),
                            project_dirpath.join(LOCAL_CONFIG_FILENAME),
                        ],
                    },
                    |_, res| assert_export(res, var_key, "\\\"\\$VAL"),
                )
            }

            #[test]
            fn ok_when_cfg_filepaths_is_not_empty() {
                let var_key = "VAR";
                let project_dirpath = tempdir().unwrap().into_path();
                let cfg_filepath = tempdir().unwrap().into_path().join(PROJECT_CONFIG_FILENAME);
                test(
                    |_| Parameters {
                        args: EnvCommandArguments {
                            cfg_filepaths: vec![cfg_filepath.clone()],
                            project_dirpath: Some(project_dirpath.clone()),
                        },
                        env: HashMap::from_iter([(
                            var_key.into(),
                            EnvVarKind::Literal("\"$VAL".into()),
                        )]),
                    },
                    |_| Expected {
                        cfg_filepaths: vec![cfg_filepath.clone()],
                    },
                    |_, res| assert_export(res, var_key, "\\\"\\$VAL"),
                )
            }

            fn assert_export(res: crate::err::Result<String>, var_key: &str, var_val: &str) {
                let out = res.unwrap();
                let expected_out = format!("export {}=\"{}\"\n", var_key, var_val);
                assert_eq!(out, expected_out);
            }

            fn test<
                P: Fn(&Context) -> Parameters,
                E: Fn(&Context) -> Expected,
                A: Fn(&Context, crate::err::Result<String>),
            >(
                create_params_fn: P,
                create_expected_fn: E,
                assert_fn: A,
            ) {
                let ctx = Context {
                    cwd: tempdir().unwrap().into_path(),
                };
                let params = create_params_fn(&ctx);
                let expected = create_expected_fn(&ctx);
                for filepath in expected.cfg_filepaths.iter() {
                    File::create(filepath).unwrap();
                }
                let cfg_loader = StubConfigLoader::new().with_stub_of_load(move |_, filepaths| {
                    assert_eq!(filepaths, expected.cfg_filepaths);
                    Ok(Config {
                        env: params.env.clone(),
                    })
                });
                let fs = StubFileSystem::new().with_stub_of_cwd({
                    let cwd = ctx.cwd.clone();
                    move |_| Ok(cwd.clone())
                });
                let cmd = EnvCommand {
                    args: params.args,
                    cfg_loader: Box::new(cfg_loader),
                    fs: Box::new(fs),
                };
                let mut out = vec![];
                let res = cmd.run(&mut out).map(|_| String::from_utf8(out).unwrap());
                assert_fn(&ctx, res);
            }
        }
    }
}
