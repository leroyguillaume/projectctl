use std::{
    fmt::{self, Debug, Formatter},
    io::Write,
};

use regex::Regex;

use crate::{
    cfg::{ConfigLoader, DefaultConfigLoader, EnvVarKind},
    cli::EnvCommandArguments,
    consts::{DEFAULT_CONFIG_FILENAME, LOCAL_CONFIG_FILENAME},
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
        let cfg_filepaths = if self.args.cfg_filepaths.is_empty() {
            let default_cfg_filepath = project_dirpath.join(DEFAULT_CONFIG_FILENAME);
            let local_cfg_filepath = project_dirpath.join(LOCAL_CONFIG_FILENAME);
            let mut cfg_filepaths = vec![];
            if default_cfg_filepath.exists() {
                cfg_filepaths.push(default_cfg_filepath);
            }
            if local_cfg_filepath.exists() {
                cfg_filepaths.push(local_cfg_filepath);
            }
            cfg_filepaths
        } else {
            self.args.cfg_filepaths
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
    use std::{
        collections::HashMap,
        fs::File,
        path::{Path, PathBuf},
    };

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
                let args = EnvCommandArguments::default_for_test();
                let cmd = EnvCommand::new(args.clone());
                assert_eq!(cmd.args, args);
            }
        }

        mod run {
            use super::*;

            struct Context<'a> {
                cwd: &'a Path,
                escaped_var_val: &'a str,
                project_dirpath: &'a Path,
                var_key: &'a str,
                var_val: &'a str,
            }

            struct Data {
                cfg_filepaths: Vec<PathBuf>,
                params: Parameters,
            }

            struct Parameters {
                args: EnvCommandArguments,
                cfg: Config,
                default_cfg_filepath: Option<PathBuf>,
                local_cfg_filepath: Option<PathBuf>,
            }

            #[test]
            fn ok_when_cfg_filepaths_is_empty_and_project_dirpath_is_undefined_and_files_do_not_exist(
            ) {
                test(
                    |_| Data {
                        cfg_filepaths: vec![],
                        params: Parameters {
                            args: EnvCommandArguments::default_for_test(),
                            cfg: Config::default(),
                            default_cfg_filepath: None,
                            local_cfg_filepath: None,
                        },
                    },
                    |_, out| {
                        assert!(out.is_empty());
                    },
                );
            }

            #[test]
            fn ok_when_cfg_filepaths_is_empty_and_project_dirpath_is_undefined() {
                test(
                    |ctx| {
                        let default_cfg_filepath = ctx.cwd.join(DEFAULT_CONFIG_FILENAME);
                        let local_cfg_filepath = ctx.cwd.join(LOCAL_CONFIG_FILENAME);
                        Data {
                            cfg_filepaths: vec![
                                default_cfg_filepath.clone(),
                                local_cfg_filepath.clone(),
                            ],
                            params: Parameters {
                                args: EnvCommandArguments::default_for_test(),
                                cfg: Config {
                                    env: HashMap::from_iter([(
                                        ctx.var_key.into(),
                                        EnvVarKind::Literal(ctx.var_val.into()),
                                    )]),
                                },
                                default_cfg_filepath: Some(default_cfg_filepath),
                                local_cfg_filepath: Some(local_cfg_filepath),
                            },
                        }
                    },
                    verify_export,
                );
            }

            #[test]
            fn ok_when_cfg_filepaths_is_not_empty_and_project_dirpath_is_undefined() {
                let cfg_filepaths = vec![PathBuf::from("/"), PathBuf::from("/etc")];
                test(
                    |ctx| Data {
                        cfg_filepaths: cfg_filepaths.clone(),
                        params: Parameters {
                            args: EnvCommandArguments {
                                cfg_filepaths: cfg_filepaths.clone(),
                                ..EnvCommandArguments::default_for_test()
                            },
                            cfg: Config {
                                env: HashMap::from_iter([(
                                    ctx.var_key.into(),
                                    EnvVarKind::Literal(ctx.var_val.into()),
                                )]),
                            },
                            default_cfg_filepath: None,
                            local_cfg_filepath: None,
                        },
                    },
                    verify_export,
                );
            }

            #[test]
            fn ok_when_cfg_filepaths_is_not_empty_and_project_dirpath_is_defined() {
                let cfg_filepaths = vec![PathBuf::from("/"), PathBuf::from("/etc")];
                test(
                    |ctx| Data {
                        cfg_filepaths: cfg_filepaths.clone(),
                        params: Parameters {
                            args: EnvCommandArguments {
                                cfg_filepaths: cfg_filepaths.clone(),
                                project_dirpath: Some(ctx.project_dirpath.to_path_buf()),
                            },
                            cfg: Config {
                                env: HashMap::from_iter([(
                                    ctx.var_key.into(),
                                    EnvVarKind::Literal(ctx.var_val.into()),
                                )]),
                            },
                            default_cfg_filepath: None,
                            local_cfg_filepath: None,
                        },
                    },
                    verify_export,
                );
            }

            #[test]
            fn ok_when_cfg_filepaths_is_empty_and_project_dirpath_is_defined() {
                test(
                    |ctx| {
                        let default_cfg_filepath =
                            ctx.project_dirpath.join(DEFAULT_CONFIG_FILENAME);
                        let local_cfg_filepath = ctx.project_dirpath.join(LOCAL_CONFIG_FILENAME);
                        Data {
                            cfg_filepaths: vec![
                                default_cfg_filepath.clone(),
                                local_cfg_filepath.clone(),
                            ],
                            params: Parameters {
                                args: EnvCommandArguments {
                                    project_dirpath: Some(ctx.project_dirpath.to_path_buf()),
                                    ..EnvCommandArguments::default_for_test()
                                },
                                cfg: Config {
                                    env: HashMap::from_iter([(
                                        ctx.var_key.into(),
                                        EnvVarKind::Literal(ctx.var_val.into()),
                                    )]),
                                },
                                default_cfg_filepath: Some(default_cfg_filepath),
                                local_cfg_filepath: Some(local_cfg_filepath),
                            },
                        }
                    },
                    verify_export,
                );
            }

            #[inline]
            fn test<D: Fn(&Context) -> Data, A: Fn(&Context, String)>(
                data_from_fn: D,
                assert_fn: A,
            ) {
                let cwd = tempdir().unwrap().into_path();
                let project_dirpath = tempdir().unwrap().into_path();
                let ctx = Context {
                    cwd: &cwd,
                    escaped_var_val: "\\\"\\$VAR",
                    project_dirpath: &project_dirpath,
                    var_key: "VAR",
                    var_val: "\"$VAR",
                };
                let data = data_from_fn(&ctx);
                if let Some(path) = data.params.default_cfg_filepath {
                    File::create(path).unwrap();
                }
                if let Some(path) = data.params.local_cfg_filepath {
                    File::create(path).unwrap();
                }
                let cfg_loader = StubConfigLoader::new().with_stub_of_load(move |_, filepaths| {
                    assert_eq!(filepaths, data.cfg_filepaths);
                    Ok(data.params.cfg.clone())
                });
                let fs = StubFileSystem::new().with_stub_of_cwd({
                    let cwd = cwd.clone();
                    move |_| Ok(cwd.clone())
                });
                let cmd = EnvCommand {
                    args: data.params.args,
                    cfg_loader: Box::new(cfg_loader),
                    fs: Box::new(fs),
                };
                let mut out = vec![];
                cmd.run(&mut out).unwrap();
                assert_fn(&ctx, String::from_utf8(out).unwrap());
            }

            #[inline]
            fn verify_export(ctx: &Context, out: String) {
                let expected_out = format!("export {}=\"{}\"\n", ctx.var_key, ctx.escaped_var_val);
                assert_eq!(out, expected_out);
            }
        }
    }
}
