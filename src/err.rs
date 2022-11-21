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
}

impl Error {
    pub fn to_return_code(&self) -> i32 {
        match self {
            Self::DestinationDirectoryAlreadyExists(_) => exitcode::IOERR,
            Self::Git(_) => exitcode::SOFTWARE,
            Self::IO(_) => exitcode::IOERR,
        }
    }
}

impl Display for Error {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result {
        match self {
            Self::DestinationDirectoryAlreadyExists(path) => {
                write!(f, "{} already exists", path.display())
            }
            Self::Git(err) => write!(f, "{}", err),
            Self::IO(err) => write!(f, "{}", err),
        }
    }
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
        }
    }

    mod to_string {
        use super::*;

        #[test]
        fn destination_directory_already_exists() {
            let path = Path::new("/");
            let str = format!("{} already exists", path.display());
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
    }
}
