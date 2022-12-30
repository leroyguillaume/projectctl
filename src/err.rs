use std::{
    fmt::{self, Display, Formatter},
    io,
    path::PathBuf,
};

use jsonschema::ValidationError;

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
    Liquid {
        cause: liquid::Error,
        src: LiquidErrorSource,
    },
    MalformedConfig {
        cause: serde_yaml::Error,
        path: PathBuf,
    },
    MalformedValues(serde_json::Error),
    NotAJsonObject,
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
            Self::Liquid { .. } => exitcode::DATAERR,
            Self::MalformedConfig { .. } => exitcode::CONFIG,
            Self::MalformedValues(_) => exitcode::USAGE,
            Self::NotAJsonObject => exitcode::USAGE,
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
            Self::Liquid { src: source, .. } => match source {
                LiquidErrorSource::File(path) => write!(f, "Unable to render {}", path.display()),
                LiquidErrorSource::Filename(filename) => {
                    write!(f, "Unable to render `{}`", filename.display())
                }
                LiquidErrorSource::Values => write!(f, "Unable to declare values"),
            },
            Self::MalformedConfig { cause, path } => write!(f, "{}: {}", path.display(), cause),
            Self::MalformedValues(err) => write!(f, "{}", err),
            Self::NotAJsonObject => write!(f, "Must be a JSON object"),
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
    Values,
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
            fn malformed_config() {
                test(
                    || Parameters {
                        err: Error::MalformedConfig {
                            cause: serde_yaml::from_str::<serde_yaml::Value>("{").unwrap_err(),
                            path: "/".into(),
                        },
                    },
                    |rc| assert_eq!(rc, exitcode::CONFIG),
                );
            }

            #[test]
            fn malformed_values() {
                test(
                    || Parameters {
                        err: Error::MalformedValues(
                            serde_json::from_str::<serde_json::Value>("{").unwrap_err(),
                        ),
                    },
                    |rc| assert_eq!(rc, exitcode::USAGE),
                );
            }

            #[test]
            fn not_a_json_object() {
                test(
                    || Parameters {
                        err: Error::NotAJsonObject,
                    },
                    |rc| assert_eq!(rc, exitcode::USAGE),
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
        fn liquid_when_src_is_values() {
            test(
                || Parameters {
                    err: Error::Liquid {
                        cause: liquid::Error::with_msg("error"),
                        src: LiquidErrorSource::Values,
                    },
                },
                |str| assert_eq!(str, "Unable to declare values"),
            );
        }

        #[test]
        fn malformed_config() {
            let yaml = "{";
            let path = Path::new("/");
            test(
                || Parameters {
                    err: Error::MalformedConfig {
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
        fn malformed_values() {
            let json = "{";
            test(
                || Parameters {
                    err: Error::MalformedValues(
                        serde_json::from_str::<serde_json::Value>(json).unwrap_err(),
                    ),
                },
                |str| {
                    let cause = serde_json::from_str::<serde_json::Value>(json).unwrap_err();
                    assert_eq!(str, cause.to_string());
                },
            )
        }

        #[test]
        fn not_a_json_object() {
            test(
                || Parameters {
                    err: Error::NotAJsonObject,
                },
                |str| assert_eq!(str, "Must be a JSON object"),
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
