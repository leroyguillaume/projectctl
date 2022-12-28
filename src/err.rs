use std::{
    fmt::{self, Display, Formatter},
    io,
    path::PathBuf,
};

use jsonschema::ValidationError;

use crate::cli::KEY_VALUE_PATTERN;

pub type Result<T> = std::result::Result<T, Error>;

#[derive(Debug)]
pub enum Error {
    Git(git2::Error),
    HomeNotFound,
    IO(io::Error),
    InvalidConfig {
        causes: Vec<ValidationError<'static>>,
        path: PathBuf,
    },
    InvalidVariable(String),
    Liquid {
        cause: liquid::Error,
        src: LiquidErrorSource,
    },
    MalformedYaml {
        cause: serde_yaml::Error,
        path: PathBuf,
    },
    TemplateNotFound(String),
    UnsupportedShell(String),
}

impl Error {
    pub fn to_return_code(&self) -> i32 {
        match self {
            Self::Git(_) => exitcode::SOFTWARE,
            Self::HomeNotFound => exitcode::UNAVAILABLE,
            Self::IO(_) => exitcode::IOERR,
            Self::InvalidConfig { .. } => exitcode::CONFIG,
            Self::InvalidVariable(_) => exitcode::USAGE,
            Self::Liquid { .. } => exitcode::DATAERR,
            Self::MalformedYaml { .. } => exitcode::CONFIG,
            Self::TemplateNotFound(_) => exitcode::DATAERR,
            Self::UnsupportedShell(_) => exitcode::DATAERR,
        }
    }
}

impl Display for Error {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        match self {
            Self::Git(err) => write!(f, "git: {}", err),
            Self::HomeNotFound => write!(f, "Home directory not found"),
            Self::IO(err) => write!(f, "{}", err),
            Self::InvalidConfig { path, .. } => {
                write!(f, "{}: Invalid configuration", path.display())
            }
            Self::InvalidVariable(key_val) => {
                write!(f, "`{}` does not match `{}`", key_val, KEY_VALUE_PATTERN)
            }
            Self::Liquid { src: source, .. } => match source {
                LiquidErrorSource::File(path) => write!(f, "Unable to render {}", path.display()),
                LiquidErrorSource::Filename(filename) => {
                    write!(f, "Unable to render `{}`", filename.display())
                }
            },
            Self::MalformedYaml { cause, path } => write!(f, "{}: {}", path.display(), cause),
            Self::TemplateNotFound(tpl) => write!(f, "Template `{}` not found", tpl),
            Self::UnsupportedShell(shell) => write!(f, "Shell `{}` is not supported", shell),
        }
    }
}

impl std::error::Error for Error {}

#[derive(Debug)]
pub enum LiquidErrorSource {
    File(PathBuf),
    Filename(PathBuf),
}

#[cfg(test)]
mod test {
    use std::path::Path;

    use super::*;

    mod error {
        use super::*;

        mod to_return_code {
            use super::*;

            struct Parameters {
                err: Error,
            }

            #[test]
            fn git() {
                test(
                    || Parameters {
                        err: Error::Git(git2::Error::new(
                            git2::ErrorCode::Ambiguous,
                            git2::ErrorClass::Callback,
                            "",
                        )),
                    },
                    |rc| assert_eq!(rc, exitcode::SOFTWARE),
                );
            }

            #[test]
            fn home_dir_not_found() {
                test(
                    || Parameters {
                        err: Error::HomeNotFound,
                    },
                    |rc| assert_eq!(rc, exitcode::UNAVAILABLE),
                );
            }

            #[test]
            fn io() {
                test(
                    || Parameters {
                        err: Error::IO(io::Error::from(io::ErrorKind::PermissionDenied)),
                    },
                    |rc| assert_eq!(rc, exitcode::IOERR),
                );
            }

            #[test]
            fn invalid_config() {
                test(
                    || Parameters {
                        err: Error::InvalidConfig {
                            causes: vec![],
                            path: "/".into(),
                        },
                    },
                    |rc| assert_eq!(rc, exitcode::CONFIG),
                );
            }

            #[test]
            fn invalid_variable() {
                test(
                    || Parameters {
                        err: Error::InvalidVariable("test".into()),
                    },
                    |rc| assert_eq!(rc, exitcode::USAGE),
                );
            }

            #[test]
            fn liquid() {
                test(
                    || Parameters {
                        err: Error::Liquid {
                            cause: liquid::Error::with_msg("error"),
                            src: LiquidErrorSource::File("/".into()),
                        },
                    },
                    |rc| assert_eq!(rc, exitcode::DATAERR),
                );
            }

            #[test]
            fn malformed_yaml() {
                test(
                    || Parameters {
                        err: Error::MalformedYaml {
                            cause: serde_yaml::from_str::<serde_yaml::Value>("{").unwrap_err(),
                            path: "/".into(),
                        },
                    },
                    |rc| assert_eq!(rc, exitcode::CONFIG),
                );
            }

            #[test]
            fn template_not_found() {
                test(
                    || Parameters {
                        err: Error::TemplateNotFound("test".into()),
                    },
                    |rc| assert_eq!(rc, exitcode::DATAERR),
                );
            }

            #[test]
            fn unsupported_shell() {
                test(
                    || Parameters {
                        err: Error::UnsupportedShell("test".into()),
                    },
                    |rc| assert_eq!(rc, exitcode::DATAERR),
                );
            }

            fn test<P: Fn() -> Parameters, A: Fn(i32)>(create_params_fn: P, assert_fn: A) {
                let params = create_params_fn();
                let rc = params.err.to_return_code();
                assert_fn(rc);
            }
        }
    }

    mod to_string {
        use super::*;

        struct Parameters {
            err: Error,
        }

        #[test]
        fn git() {
            let err_code = git2::ErrorCode::Ambiguous;
            let err_class = git2::ErrorClass::Callback;
            let err_msg = "";
            test(
                || Parameters {
                    err: Error::Git(git2::Error::new(err_code, err_class, err_msg)),
                },
                |str| {
                    let expected_str =
                        format!("git: {}", git2::Error::new(err_code, err_class, err_msg));
                    assert_eq!(str, expected_str);
                },
            );
        }

        #[test]
        fn home_dir_not_found() {
            test(
                || Parameters {
                    err: Error::HomeNotFound,
                },
                |str| assert_eq!(str, "Home directory not found"),
            );
        }

        #[test]
        fn io() {
            let kind = io::ErrorKind::PermissionDenied;
            test(
                || Parameters {
                    err: Error::IO(io::Error::from(kind)),
                },
                |str| {
                    let expected_str = io::Error::from(kind).to_string();
                    assert_eq!(str, expected_str);
                },
            );
        }

        #[test]
        fn invalid_config() {
            let path = Path::new("/");
            let expected_str = format!("{}: Invalid configuration", path.display());
            test(
                || Parameters {
                    err: Error::InvalidConfig {
                        causes: vec![],
                        path: path.to_path_buf(),
                    },
                },
                |str| assert_eq!(str, expected_str),
            );
        }

        #[test]
        fn invalid_variable() {
            let key_val = "test";
            let expected_str = format!("`{}` does not match `{}`", key_val, KEY_VALUE_PATTERN);
            test(
                || Parameters {
                    err: Error::InvalidVariable(key_val.into()),
                },
                |str| assert_eq!(str, expected_str),
            );
        }

        #[test]
        fn liquid_when_src_is_file() {
            let path = Path::new("/");
            let expected_str = format!("Unable to render {}", path.display());
            test(
                || Parameters {
                    err: Error::Liquid {
                        cause: liquid::Error::with_msg("error"),
                        src: LiquidErrorSource::File(path.to_path_buf()),
                    },
                },
                |str| assert_eq!(str, expected_str),
            );
        }

        #[test]
        fn liquid_when_src_is_filename() {
            let filename = Path::new("test");
            let expected_str = format!("Unable to render `{}`", filename.display());
            test(
                || Parameters {
                    err: Error::Liquid {
                        cause: liquid::Error::with_msg("error"),
                        src: LiquidErrorSource::Filename(filename.to_path_buf()),
                    },
                },
                |str| assert_eq!(str, expected_str),
            );
        }

        #[test]
        fn malformed_yaml() {
            let yaml = "{";
            let path = Path::new("/");
            test(
                || Parameters {
                    err: Error::MalformedYaml {
                        cause: serde_yaml::from_str::<serde_yaml::Value>(yaml).unwrap_err(),
                        path: path.to_path_buf(),
                    },
                },
                |str| {
                    let cause = serde_yaml::from_str::<serde_yaml::Value>(yaml).unwrap_err();
                    let expected_str = format!("{}: {}", path.display(), cause);
                    assert_eq!(str, expected_str);
                },
            );
        }

        #[test]
        fn template_not_found() {
            let tpl = "test";
            let expected_str = format!("Template `{}` not found", tpl);
            test(
                || Parameters {
                    err: Error::TemplateNotFound(tpl.into()),
                },
                |str| assert_eq!(str, expected_str),
            );
        }

        #[test]
        fn unsupported_shell() {
            let shell = "test";
            let expected_str = format!("Shell `{}` is not supported", shell);
            test(
                || Parameters {
                    err: Error::UnsupportedShell(shell.into()),
                },
                |str| assert_eq!(str, expected_str),
            );
        }

        fn test<P: Fn() -> Parameters, A: Fn(String)>(create_params_fn: P, assert_fn: A) {
            let params = create_params_fn();
            let str = params.err.to_string();
            assert_fn(str);
        }
    }
}
