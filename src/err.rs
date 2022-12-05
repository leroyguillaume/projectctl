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
    InvalidConfig(Vec<ValidationError<'static>>),
    Liquid(liquid::Error),
    MalformedYaml(serde_yaml::Error),
    TemplateNotFound(String),
}

impl Error {
    pub fn to_return_code(&self) -> i32 {
        match self {
            Self::DestinationDirectoryAlreadyExists(_) => exitcode::IOERR,
            Self::Git(_) => exitcode::SOFTWARE,
            Self::IO(_) => exitcode::IOERR,
            Self::InvalidConfig(_) => exitcode::CONFIG,
            Self::Liquid(_) => exitcode::SOFTWARE,
            Self::MalformedYaml(_) => exitcode::CONFIG,
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
            Self::InvalidConfig(_) => write!(f, "Invalid configuration"),
            Self::Liquid(err) => write!(f, "{}", err),
            Self::MalformedYaml(err) => write!(f, "{}", err),
            Self::TemplateNotFound(tpl) => write!(f, "Template `{}` not found", tpl),
        }
    }
}

impl std::error::Error for Error {}

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
                Error::DestinationDirectoryAlreadyExists(PathBuf::from("/")),
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
                Error::InvalidConfig(vec![]),
                exitcode::CONFIG
            );
            test!(
                liquid,
                Error::Liquid(liquid::Error::with_msg("error")),
                exitcode::SOFTWARE
            );
            test!(
                malformed_yaml,
                Error::MalformedYaml(serde_yaml::from_str::<serde_yaml::Value>("{").unwrap_err()),
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
            let err = Error::InvalidConfig(vec![]);
            assert_eq!(err.to_string(), "Invalid configuration");
        }

        #[test]
        fn liquid() {
            let cause = liquid::Error::with_msg("error");
            let str = cause.to_string();
            let err = Error::Liquid(cause);
            assert_eq!(err.to_string(), str);
        }

        #[test]
        fn malformed_yaml() {
            let cause = serde_yaml::from_str::<serde_yaml::Value>("{").unwrap_err();
            let str = cause.to_string();
            let err = Error::MalformedYaml(cause);
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
