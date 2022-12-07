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
        info!("Loading configuration from {}", filepath.display());
        let file = fs.open(filepath, OpenOptions::new().read(true).to_owned())?;
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
    use std::fs::write;
    use tempfile::tempdir;

    use crate::fs::StubFileSystem;

    use super::*;

    mod default_config_loader {
        use super::*;

        mod load {
            use super::*;

            struct Context<'a> {
                cfg_filepath1: &'a Path,
            }

            struct Parameters {
                cfg_file_content1: &'static str,
                cfg_file_content2: &'static str,
            }

            #[test]
            fn err_if_yaml_is_malformed() {
                test(
                    Parameters {
                        cfg_file_content1: "{",
                        cfg_file_content2: "",
                    },
                    |ctx, res| match res.unwrap_err() {
                        Error::MalformedYaml { path, .. } => assert_eq!(path, ctx.cfg_filepath1),
                        err => panic!("expected MalformedYaml (actual: {:?})", err),
                    },
                );
            }

            #[test]
            fn err_if_config_is_invalid() {
                test(
                    Parameters {
                        cfg_file_content1: "key: value",
                        cfg_file_content2: "",
                    },
                    |ctx, res| match res.unwrap_err() {
                        Error::InvalidConfig { path, .. } => assert_eq!(path, ctx.cfg_filepath1),
                        err => panic!("expected InvalidConfig (actual: {:?})", err),
                    },
                );
            }

            #[test]
            fn ok() {
                test(
                    Parameters {
                        cfg_file_content1: include_str!("../examples/projectctl.yml"),
                        cfg_file_content2: "env:\n  HOST: 127.0.0.1",
                    },
                    |_, res| {
                        let cfg = res.unwrap();
                        let expected_cfg = Config {
                            env: HashMap::from_iter([
                                ("DEBUG".into(), EnvVarKind::Literal("true".into())),
                                ("HOST".into(), EnvVarKind::Literal("127.0.0.1".into())),
                                ("PORT".into(), EnvVarKind::Literal("9090".into())),
                            ]),
                        };
                        assert_eq!(cfg, expected_cfg);
                    },
                );
            }

            #[inline]
            fn test<A: Fn(&Context, Result<Config>)>(params: Parameters, assert_fn: A) {
                let dirpath = tempdir().unwrap().into_path();
                let cfg_filepath1 = dirpath.join("projectctl1.yml");
                let cfg_filepath2 = dirpath.join("projectctl2.yml");
                write(&cfg_filepath1, params.cfg_file_content1).unwrap();
                write(&cfg_filepath2, params.cfg_file_content2).unwrap();
                let fs = StubFileSystem::new().with_stub_of_open({
                    let cfg_filepath1 = cfg_filepath1.clone();
                    let cfg_filepath2 = cfg_filepath2.clone();
                    move |i, path, opts| {
                        if i == 0 {
                            assert_eq!(path, cfg_filepath1);
                        } else if i == 1 {
                            assert_eq!(path, cfg_filepath2);
                        } else {
                            panic!("unexpected invocation of open");
                        }
                        opts.open(path).map_err(Error::IO)
                    }
                });
                let loader = DefaultConfigLoader { fs: Box::new(fs) };
                let res = loader.load(&[cfg_filepath1.clone(), cfg_filepath2]);
                let ctx = Context {
                    cfg_filepath1: &cfg_filepath1,
                };
                assert_fn(&ctx, res);
            }
        }
    }
}
