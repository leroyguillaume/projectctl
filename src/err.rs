use std::{
    fmt::{self, Display, Formatter},
    io,
    path::PathBuf,
};

use jsonschema::ValidationError;

pub type Result<T> = std::result::Result<T, Error>;

#[derive(Debug)]
pub enum Error {
    DestinationDirectoryAlreadyExists(PathBuf),
    Git(git2::Error),
    IO(io::Error),
    InvalidConfig {
        causes: Vec<ValidationError<'static>>,
        path: PathBuf,
    },
    Liquid {
        cause: liquid::Error,
        src: LiquidErrorSource,
    },
    MalformedYaml {
        cause: serde_yaml::Error,
        path: PathBuf,
    },
    TemplateNotFound(String),
}

impl Error {
    pub fn to_return_code(&self) -> i32 {
        match self {
            Self::DestinationDirectoryAlreadyExists(_) => exitcode::IOERR,
            Self::Git(_) => exitcode::SOFTWARE,
            Self::IO(_) => exitcode::IOERR,
            Self::InvalidConfig { .. } => exitcode::CONFIG,
            Self::Liquid { .. } => exitcode::SOFTWARE,
            Self::MalformedYaml { .. } => exitcode::CONFIG,
            Self::TemplateNotFound(_) => exitcode::CONFIG,
        }
    }
}

impl Display for Error {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        match self {
            Self::DestinationDirectoryAlreadyExists(path) => {
                write!(f, "Directory {} already exists", path.display())
            }
            Self::Git(err) => write!(f, "git: {}", err),
            Self::IO(err) => write!(f, "{}", err),
            Self::InvalidConfig { path, .. } => {
                write!(f, "{}: Invalid configuration", path.display())
            }
            Self::Liquid { src: source, .. } => match source {
                LiquidErrorSource::File(path) => write!(f, "Unable to render {}", path.display()),
                LiquidErrorSource::Filename(filename) => {
                    write!(f, "Unable to render `{}`", filename.display())
                }
            },
            Self::MalformedYaml { cause, path } => write!(f, "{}: {}", path.display(), cause),
            Self::TemplateNotFound(tpl) => write!(f, "Template `{}` not found", tpl),
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
            fn destination_directory_already_exists() {
                test(
                    || Parameters {
                        err: Error::DestinationDirectoryAlreadyExists("/".into()),
                    },
                    |rc| assert_eq!(rc, exitcode::IOERR),
                );
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
                    |rc| assert_eq!(rc, exitcode::SOFTWARE),
                );
            }

            #[test]
            fn malformed() {
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
                    |rc| assert_eq!(rc, exitcode::CONFIG),
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
        fn destination_directory_already_exists() {
            let path = Path::new("/");
            let expected_str = format!("Directory {} already exists", path.display());
            test(
                || Parameters {
                    err: Error::DestinationDirectoryAlreadyExists(path.to_path_buf()),
                },
                |str| assert_eq!(str, expected_str),
            );
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

        fn test<P: Fn() -> Parameters, A: Fn(String)>(create_params_fn: P, assert_fn: A) {
            let params = create_params_fn();
            let str = params.err.to_string();
            assert_fn(str);
        }
    }
}
