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

            macro_rules! test {
                ($ident:ident, $err:expr, $rc:expr) => {
                    #[test]
                    fn $ident() {
                        assert_eq!($err.to_return_code(), $rc);
                    }
                };
            }

            test!(
                destination_directory_already_exists,
                Error::DestinationDirectoryAlreadyExists("/".into()),
                exitcode::IOERR
            );
            test!(
                git,
                Error::Git(git2::Error::new(
                    git2::ErrorCode::Ambiguous,
                    git2::ErrorClass::Callback,
                    "",
                )),
                exitcode::SOFTWARE
            );
            test!(
                io,
                Error::IO(io::Error::from(io::ErrorKind::PermissionDenied)),
                exitcode::IOERR
            );
            test!(
                invalid_config,
                Error::InvalidConfig {
                    causes: vec![],
                    path: "/".into(),
                },
                exitcode::CONFIG
            );
            test!(
                liquid,
                Error::Liquid {
                    cause: liquid::Error::with_msg("error"),
                    src: LiquidErrorSource::File("/".into()),
                },
                exitcode::SOFTWARE
            );
            test!(
                malformed_yaml,
                Error::MalformedYaml {
                    cause: serde_yaml::from_str::<serde_yaml::Value>("{").unwrap_err(),
                    path: "/".into(),
                },
                exitcode::CONFIG
            );
            test!(
                template_not_found,
                Error::TemplateNotFound("test".into()),
                exitcode::CONFIG
            );
        }
    }

    mod to_string {
        use super::*;

        #[test]
        fn destination_directory_already_exists() {
            let path = Path::new("/");
            let str = format!("Directory {} already exists", path.display());
            let err = Error::DestinationDirectoryAlreadyExists(path.into());
            assert_eq!(err.to_string(), str);
        }

        #[test]
        fn git() {
            let cause =
                git2::Error::new(git2::ErrorCode::Ambiguous, git2::ErrorClass::Callback, "");
            let str = format!("git: {}", cause);
            let err = Error::Git(cause);
            assert_eq!(err.to_string(), str);
        }

        #[test]
        fn io() {
            let cause = io::Error::from(io::ErrorKind::PermissionDenied);
            let str = cause.to_string();
            let err = Error::IO(cause);
            assert_eq!(err.to_string(), str);
        }

        #[test]
        fn invalid_config() {
            let path = PathBuf::from("/");
            let str = format!("{}: Invalid configuration", path.display());
            let err = Error::InvalidConfig {
                causes: vec![],
                path,
            };
            assert_eq!(err.to_string(), str);
        }

        #[test]
        fn liquid_when_src_is_file() {
            let path = PathBuf::from("/");
            let cause = liquid::Error::with_msg("error");
            let str = format!("Unable to render {}", path.display());
            let err = Error::Liquid {
                cause,
                src: LiquidErrorSource::File(path),
            };
            assert_eq!(err.to_string(), str);
        }

        #[test]
        fn liquid_when_src_is_filename() {
            let filename = PathBuf::from("test");
            let cause = liquid::Error::with_msg("error");
            let str = format!("Unable to render `{}`", filename.display());
            let err = Error::Liquid {
                cause,
                src: LiquidErrorSource::Filename(filename),
            };
            assert_eq!(err.to_string(), str);
        }

        #[test]
        fn malformed_yaml() {
            let path = PathBuf::from("/");
            let cause = serde_yaml::from_str::<serde_yaml::Value>("{").unwrap_err();
            let str = format!("{}: {}", path.display(), cause);
            let err = Error::MalformedYaml { cause, path };
            assert_eq!(err.to_string(), str);
        }

        #[test]
        fn template_not_found() {
            let tpl = "test";
            let str = format!("Template `{}` not found", tpl);
            let err = Error::TemplateNotFound(tpl.into());
            assert_eq!(err.to_string(), str);
        }
    }
}
