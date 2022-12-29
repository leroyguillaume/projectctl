use std::{
    borrow::Cow,
    collections::HashMap,
    fs::OpenOptions,
    path::{Path, PathBuf},
};

use jsonschema::{JSONSchema, ValidationError};
use log::{debug, info, trace, warn};
use serde_json::Value;
#[cfg(test)]
use stub_trait::stub;

use crate::{
    err::{Error, Result},
    fs::{DefaultFileSystem, FileSystem},
};

pub const JSON_SCHEMA: &str = include_str!("../resources/main/config.schema.json");

const ENV_KEY: &str = "env";

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct Config {
    pub env: HashMap<String, EnvVarKind>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum EnvVarKind {
    Literal(String),
}

#[cfg_attr(test, stub)]
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
                let val = match val {
                    Value::Bool(val) => val.to_string(),
                    Value::Number(val) => val.to_string(),
                    Value::String(val) => val,
                    _ => {
                        warn!("Invalid value for `{}.{}`", ENV_KEY, key);
                        continue;
                    }
                };
                let previous = cfg.env.insert(key.clone(), EnvVarKind::Literal(val));
                if previous.is_some() {
                    debug!("Configuration of environment variable `{}` overriden", key);
                } else {
                    debug!("Configuration of environment variable `{}` loaded", key);
                }
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
        let cfg_val: Value =
            serde_yaml::from_reader(file).map_err(|cause| Error::MalformedYaml {
                cause,
                path: filepath.to_path_buf(),
            })?;
        debug!("Validating configuration");
        schema.validate(&cfg_val).map_err(|iter| {
            let causes = iter
                .map(|err| ValidationError {
                    instance: Cow::Owned(err.instance.into_owned()),
                    instance_path: err.instance_path,
                    kind: err.kind,
                    schema_path: err.schema_path,
                })
                .collect();
            Error::InvalidConfig {
                causes,
                path: filepath.to_path_buf(),
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
                cfg1_content: String,
                cfg2_content: String,
            }

            #[test]
            fn err_when_yml_is_malformed() {
                test(
                    |_| Parameters {
                        cfg1_content: "{".into(),
                        cfg2_content: "".into(),
                    },
                    |ctx, res| match res.unwrap_err() {
                        Error::MalformedYaml { path, .. } => assert_eq!(path, ctx.cfg1_filepath),
                        err => panic!("expected MalformedYaml (actual: {:?})", err),
                    },
                );
            }

            #[test]
            fn err_when_cfg_is_invalid() {
                test(
                    |_| Parameters {
                        cfg1_content: "key: value".into(),
                        cfg2_content: "".into(),
                    },
                    |ctx, res| match res.unwrap_err() {
                        Error::InvalidConfig { path, .. } => assert_eq!(path, ctx.cfg1_filepath),
                        err => panic!("expected InvalidConfig (actual: {:?})", err),
                    },
                );
            }

            #[test]
            fn ok_when_files_are_empty() {
                test(
                    |_| Parameters {
                        cfg1_content: "---".into(),
                        cfg2_content: "---".into(),
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
                        cfg1_content: format!("{}:\n", ENV_KEY),
                        cfg2_content: "---".into(),
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
            fn ok() {
                let var1_key = "VAR1";
                let var1_val1 = "VAL1-1";
                let var1_val2 = "VAL1-2";
                let var2_key = "VAR2";
                let var2_val = "VAL2";
                let var3_key = "VAR3";
                let var3_val = "VAL3";
                test(
                    |_| Parameters {
                        cfg1_content: format!(
                            "{}:\n  {}: {}\n  {}: {}",
                            ENV_KEY, var1_key, var1_val1, var2_key, var2_val
                        ),
                        cfg2_content: format!(
                            "{}:\n  {}: {}\n  {}: {}",
                            ENV_KEY, var1_key, var1_val2, var3_key, var3_val
                        ),
                    },
                    |_, res| {
                        let expected_cfg = Config {
                            env: HashMap::from_iter([
                                (var1_key.into(), EnvVarKind::Literal(var1_val2.into())),
                                (var2_key.into(), EnvVarKind::Literal(var2_val.into())),
                                (var3_key.into(), EnvVarKind::Literal(var3_val.into())),
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
