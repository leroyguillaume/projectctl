use std::{
    borrow::Cow,
    collections::HashMap,
    fs::OpenOptions,
    path::{Path, PathBuf},
};

use jsonschema::{JSONSchema, ValidationError};
use log::{debug, info, trace, warn};
use serde_json::Value;

use crate::{
    err::{Error, ErrorKind, Result},
    fs::{DefaultFileSystem, FileSystem},
};

pub const JSON_SCHEMA: &str = include_str!("../resources/main/config.schema.json");

const ENV_KEY: &str = "env";
const ENV_RUN_KEY: &str = "run";

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct Config {
    pub env: HashMap<String, EnvVarKind>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum EnvVarKind {
    Literal(String),
    Run(String),
}

#[cfg_attr(test, stub_trait::stub)]
pub trait ConfigLoader {
    fn load(&self, filepaths: &[PathBuf]) -> Result<Config>;
}

pub struct DefaultConfigLoader {
    fs: Box<dyn FileSystem>,
}

impl DefaultConfigLoader {
    pub fn new() -> Self {
        DefaultConfigLoader {
            fs: Box::new(DefaultFileSystem),
        }
    }

    #[inline]
    fn load_env(env_val: Value, cfg: &mut Config) {
        trace!("Loading environment configuration");
        if let Value::Object(env_val) = env_val {
            for (key, val) in env_val.into_iter() {
                let kind = if let Some(kind) = Self::load_var_kind(&key, val) {
                    kind
                } else {
                    continue;
                };
                let previous = cfg.env.insert(key.clone(), kind);
                if previous.is_some() {
                    debug!("Configuration of environment variable `{}` overriden", key);
                } else {
                    debug!("Configuration of environment variable `{}` loaded", key);
                }
            }
        }
    }

    #[inline]
    fn load_var_kind(key: &str, val: Value) -> Option<EnvVarKind> {
        match val {
            Value::Bool(val) => Some(EnvVarKind::Literal(val.to_string())),
            Value::Number(val) => Some(EnvVarKind::Literal(val.to_string())),
            Value::Object(val) => {
                if let Some(val) = val.get(ENV_RUN_KEY).and_then(|val| val.as_str()) {
                    Some(EnvVarKind::Run(val.trim().into()))
                } else {
                    warn!("Invalid value for `{}.{}`", ENV_KEY, key);
                    None
                }
            }
            Value::String(val) => Some(EnvVarKind::Literal(val)),
            _ => {
                warn!("Invalid value for `{}.{}`", ENV_KEY, key);
                None
            }
        }
    }

    #[inline]
    fn load_file(
        filepath: &Path,
        cfg: &mut Config,
        schema: &JSONSchema,
        fs: &dyn FileSystem,
    ) -> Result<()> {
        info!("Loading configuration");
        let file = fs.open(filepath, OpenOptions::new().read(true).to_owned(), false)?;
        debug!("Loading file {}", filepath.display());
        let cfg_val: Value = serde_yaml::from_reader(file).map_err(|err| Error {
            kind: ErrorKind::MalformedConfig(err),
            msg: format!("Unable to load configuration file {}", filepath.display()),
        })?;
        debug!("Validating configuration");
        schema.validate(&cfg_val).map_err(|iter| {
            let errs = iter
                .map(|err| ValidationError {
                    instance: Cow::Owned(err.instance.into_owned()),
                    instance_path: err.instance_path,
                    kind: err.kind,
                    schema_path: err.schema_path,
                })
                .collect();
            Error {
                kind: ErrorKind::InvalidConfig(errs),
                msg: format!("Configuration file {} is invalid", filepath.display()),
            }
        })?;
        if let Value::Object(cfg_val) = cfg_val {
            for (key, val) in cfg_val.into_iter() {
                match key.as_str() {
                    ENV_KEY => Self::load_env(val, cfg),
                    _ => warn!(
                        "Unexpected configuration key `{}` in {}",
                        key,
                        filepath.display()
                    ),
                }
            }
        }
        Ok(())
    }
}

impl ConfigLoader for DefaultConfigLoader {
    fn load(&self, filepaths: &[PathBuf]) -> Result<Config> {
        let mut cfg = Config::default();
        trace!("Loading configuration JSON schema");
        let schema_val = serde_json::from_str(JSON_SCHEMA).unwrap();
        trace!("Compiling JSON schema");
        let schema = JSONSchema::compile(&schema_val).unwrap();
        for filepath in filepaths {
            Self::load_file(filepath, &mut cfg, &schema, self.fs.as_ref())?;
        }
        Ok(cfg)
    }
}

#[cfg(test)]
mod test {
    use std::{fs::write, path::PathBuf};
    use tempfile::tempdir;

    use super::*;

    mod default_config_loader {
        use super::*;

        mod load {
            use super::*;

            struct Context {
                cfg1_filepath: PathBuf,
                cfg2_filepath: PathBuf,
            }

            struct Parameters {
                cfg1_content: &'static str,
                cfg2_content: &'static str,
            }

            #[test]
            fn err_when_yml_is_malformed() {
                test(
                    |_| Parameters {
                        cfg1_content: "{",
                        cfg2_content: "",
                    },
                    |_, res| match res.unwrap_err().kind {
                        ErrorKind::MalformedConfig(_) => (),
                        kind => panic!("expected MalformedConfig (actual: {:?})", kind),
                    },
                );
            }

            #[test]
            fn err_when_cfg_is_invalid() {
                test(
                    |_| Parameters {
                        cfg1_content: "key: value",
                        cfg2_content: "",
                    },
                    |_, res| match res.unwrap_err().kind {
                        ErrorKind::InvalidConfig(_) => (),
                        kind => panic!("expected InvalidConfig (actual: {:?})", kind),
                    },
                );
            }

            #[test]
            fn ok_when_files_are_empty() {
                test(
                    |_| Parameters {
                        cfg1_content: "---",
                        cfg2_content: "---",
                    },
                    |_, res| {
                        let expected_cfg = Config {
                            env: HashMap::new(),
                        };
                        let cfg = res.unwrap();
                        assert_eq!(cfg, expected_cfg);
                    },
                );
            }

            #[test]
            fn ok_when_env_is_empty() {
                test(
                    |_| Parameters {
                        cfg1_content: "env:",
                        cfg2_content: "---",
                    },
                    |_, res| {
                        let expected_cfg = Config {
                            env: HashMap::new(),
                        };
                        let cfg = res.unwrap();
                        assert_eq!(cfg, expected_cfg);
                    },
                );
            }

            #[test]
            fn ok_when_files_are_not_empty() {
                test(
                    |_| Parameters {
                        cfg1_content: include_str!("../resources/test/config/cfg1.yml"),
                        cfg2_content: include_str!("../resources/test/config/cfg2.yml"),
                    },
                    |_, res| {
                        let expected_cfg = Config {
                            env: HashMap::from_iter([
                                ("VAR_BOOL".into(), EnvVarKind::Literal("false".into())),
                                ("VAR_INT".into(), EnvVarKind::Literal("1".into())),
                                ("VAR_FLOAT".into(), EnvVarKind::Literal("1.1".into())),
                                ("VAR_STR".into(), EnvVarKind::Literal("str".into())),
                                ("VAR_RUN".into(), EnvVarKind::Run("echo test".into())),
                            ]),
                        };
                        let cfg = res.unwrap();
                        assert_eq!(cfg, expected_cfg);
                    },
                );
            }

            fn test<P: Fn(&Context) -> Parameters, A: Fn(&Context, Result<Config>)>(
                create_params_fn: P,
                assert_fn: A,
            ) {
                let dirpath = tempdir().unwrap().into_path();
                let ctx = Context {
                    cfg1_filepath: dirpath.join("cfg1.yml"),
                    cfg2_filepath: dirpath.join("cfg2.yml"),
                };
                let params = create_params_fn(&ctx);
                write(&ctx.cfg1_filepath, params.cfg1_content).unwrap();
                write(&ctx.cfg2_filepath, params.cfg2_content).unwrap();
                let loader = DefaultConfigLoader {
                    fs: Box::new(DefaultFileSystem),
                };
                let res = loader.load(&[ctx.cfg1_filepath.clone(), ctx.cfg2_filepath.clone()]);
                assert_fn(&ctx, res);
            }
        }
    }
}
