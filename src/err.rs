use std::{
    fmt::{Display, Formatter, Result},
    io,
    path::PathBuf,
};

#[derive(Debug)]
pub enum Error {
    DestinationDirectoryAlreadyExists(PathBuf),
    Git(git2::Error),
    IO(io::Error),
    Liquid(liquid::Error),
    TemplateNotFound(String),
}

impl Error {
    pub fn to_return_code(&self) -> i32 {
        match self {
            Self::DestinationDirectoryAlreadyExists(_) => exitcode::IOERR,
            Self::Git(_) => exitcode::SOFTWARE,
            Self::IO(_) => exitcode::IOERR,
            Self::Liquid(_) => exitcode::SOFTWARE,
            Self::TemplateNotFound(_) => exitcode::CONFIG,
        }
    }
}

impl Display for Error {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result {
        match self {
            Self::DestinationDirectoryAlreadyExists(path) => {
                write!(f, "directory {} already exists", path.display())
            }
            Self::Git(err) => write!(f, "{}", err),
            Self::IO(err) => write!(f, "{}", err),
            Self::Liquid(err) => write!(f, "{}", err),
            Self::TemplateNotFound(tpl) => write!(f, "template '{}' not found", tpl),
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
                liquid,
                Error::Liquid(liquid::Error::with_msg("error")),
                exitcode::SOFTWARE
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
            let str = format!("directory {} already exists", path.display());
            let err = Error::DestinationDirectoryAlreadyExists(path.into());
            assert_eq!(err.to_string(), str);
        }

        #[test]
        fn git() {
            let cause =
                git2::Error::new(git2::ErrorCode::Ambiguous, git2::ErrorClass::Callback, "");
            let str = cause.to_string();
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
        fn liquid() {
            let cause = liquid::Error::with_msg("error");
            let str = cause.to_string();
            let err = Error::Liquid(cause);
            assert_eq!(err.to_string(), str);
        }

        #[test]
        fn template_not_found() {
            let tpl = "test";
            let str = format!("template '{}' not found", tpl);
            let err = Error::TemplateNotFound(tpl.into());
            assert_eq!(err.to_string(), str);
        }
    }
}
