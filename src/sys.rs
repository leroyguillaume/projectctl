use std::{
    collections::HashMap,
    env::{var, vars, VarError},
};

use log::trace;

#[cfg_attr(test, stub_trait::stub)]
pub trait System {
    fn env_var(&self, key: &str) -> Option<String>;

    fn env_vars(&self) -> HashMap<String, String>;
}

pub struct DefaultSystem;

impl System for DefaultSystem {
    fn env_var(&self, key: &str) -> Option<String> {
        trace!("Fetching environment variable `{}`", key);
        match var(key) {
            Ok(val) => Some(val),
            Err(err) => match err {
                VarError::NotPresent => {
                    trace!("Environment variable `{}` is not defined", key);
                    None
                }
                VarError::NotUnicode(val) => Some(val.to_string_lossy().to_string()),
            },
        }
    }

    fn env_vars(&self) -> HashMap<String, String> {
        trace!("Fetching all environment variables");
        vars().collect()
    }
}

#[cfg(test)]
mod test {
    use std::env::vars;

    use rand::{distributions::Alphanumeric, thread_rng, Rng};

    use super::*;

    mod default_system {
        use super::*;

        mod env_var {
            use super::*;

            struct Parameters {
                key: String,
            }

            #[test]
            fn none() {
                let vars: Vec<(String, String)> = vars().collect();
                let mut key: String = thread_rng()
                    .sample_iter(&Alphanumeric)
                    .take(5)
                    .map(char::from)
                    .collect();
                while vars.iter().any(|(var_key, _)| *var_key == key) {
                    key = thread_rng()
                        .sample_iter(&Alphanumeric)
                        .take(5)
                        .map(char::from)
                        .collect();
                }
                test(
                    || Parameters { key: key.clone() },
                    |res| assert!(res.is_none()),
                );
            }

            #[test]
            fn some() {
                let (key, val) = vars().next().unwrap();
                test(
                    || Parameters { key: key.clone() },
                    |res| assert_eq!(res.unwrap(), val),
                );
            }

            fn test<P: Fn() -> Parameters, A: Fn(Option<String>)>(
                create_params_fn: P,
                assert_fn: A,
            ) {
                let params = create_params_fn();
                let res = DefaultSystem.env_var(&params.key);
                assert_fn(res);
            }
        }

        mod env_vars {
            use super::*;

            #[test]
            fn ok() {
                let expected_vars: HashMap<String, String> = vars().collect();
                let vars = DefaultSystem.env_vars();
                assert_eq!(vars, expected_vars);
            }
        }
    }
}
