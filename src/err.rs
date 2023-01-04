use std::{
    fmt::{self, Display, Formatter},
    io,
};

use jsonschema::ValidationError;

pub type Result<T> = std::result::Result<T, Error>;

#[derive(Debug)]
pub struct Error {
    pub kind: ErrorKind,
    pub msg: String,
}

impl Display for Error {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        match &self.kind {
            ErrorKind::Git(err) => write!(f, "{}: {}", self.msg, err),
            ErrorKind::IO(err) => write!(f, "{}: {}", self.msg, err),
            ErrorKind::Liquid(err) => write!(f, "{}: {}", self.msg, err),
            ErrorKind::MalformedConfig(err) => write!(f, "{}: {}", self.msg, err),
            ErrorKind::MalformedValues(err) => write!(f, "{}: {}", self.msg, err),
            _ => write!(f, "{}", self.msg),
        }
    }
}

impl Error {
    pub fn to_return_code(&self) -> i32 {
        match self.kind {
            ErrorKind::Git(_) => exitcode::SOFTWARE,
            ErrorKind::HomeNotFound => exitcode::UNAVAILABLE,
            ErrorKind::IO(_) => exitcode::IOERR,
            ErrorKind::InvalidConfig(_) => exitcode::CONFIG,
            ErrorKind::Liquid(_) => exitcode::DATAERR,
            ErrorKind::MalformedConfig(_) => exitcode::CONFIG,
            ErrorKind::MalformedValues(_) => exitcode::USAGE,
            ErrorKind::NotAJSONObject => exitcode::USAGE,
            ErrorKind::ScriptFailed { .. } => exitcode::IOERR,
            ErrorKind::TemplateNotFound => exitcode::DATAERR,
            ErrorKind::UnsupportedShell => exitcode::DATAERR,
        }
    }
}

impl std::error::Error for Error {}

#[derive(Debug)]
pub enum ErrorKind {
    Git(git2::Error),
    HomeNotFound,
    IO(io::Error),
    InvalidConfig(Vec<ValidationError<'static>>),
    Liquid(liquid::Error),
    MalformedConfig(serde_yaml::Error),
    MalformedValues(serde_json::Error),
    NotAJSONObject,
    ScriptFailed {
        rc: Option<i32>,
        stderr: String,
        stdout: String,
    },
    TemplateNotFound,
    UnsupportedShell,
}

#[cfg(test)]
mod test {
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
                        err: Error {
                            kind: ErrorKind::Git(git2::Error::new(
                                git2::ErrorCode::Ambiguous,
                                git2::ErrorClass::Callback,
                                "",
                            )),
                            msg: "error".into(),
                        },
                    },
                    |rc| assert_eq!(rc, exitcode::SOFTWARE),
                );
            }

            #[test]
            fn home_dir_not_found() {
                test(
                    || Parameters {
                        err: Error {
                            kind: ErrorKind::HomeNotFound,
                            msg: "error".into(),
                        },
                    },
                    |rc| assert_eq!(rc, exitcode::UNAVAILABLE),
                );
            }

            #[test]
            fn io() {
                test(
                    || Parameters {
                        err: Error {
                            kind: ErrorKind::IO(io::Error::from(io::ErrorKind::PermissionDenied)),
                            msg: "error".into(),
                        },
                    },
                    |rc| assert_eq!(rc, exitcode::IOERR),
                );
            }

            #[test]
            fn invalid_config() {
                test(
                    || Parameters {
                        err: Error {
                            kind: ErrorKind::InvalidConfig(vec![]),
                            msg: "error".into(),
                        },
                    },
                    |rc| assert_eq!(rc, exitcode::CONFIG),
                );
            }

            #[test]
            fn liquid() {
                test(
                    || Parameters {
                        err: Error {
                            kind: ErrorKind::Liquid(liquid::Error::with_msg("error")),
                            msg: "error".into(),
                        },
                    },
                    |rc| assert_eq!(rc, exitcode::DATAERR),
                );
            }

            #[test]
            fn malformed_config() {
                test(
                    || Parameters {
                        err: Error {
                            kind: ErrorKind::MalformedConfig(
                                serde_yaml::from_str::<serde_yaml::Value>("{").unwrap_err(),
                            ),
                            msg: "error".into(),
                        },
                    },
                    |rc| assert_eq!(rc, exitcode::CONFIG),
                );
            }

            #[test]
            fn malformed_values() {
                test(
                    || Parameters {
                        err: Error {
                            kind: ErrorKind::MalformedValues(
                                serde_json::from_str::<serde_json::Value>("{").unwrap_err(),
                            ),
                            msg: "error".into(),
                        },
                    },
                    |rc| assert_eq!(rc, exitcode::USAGE),
                );
            }

            #[test]
            fn not_a_json_object() {
                test(
                    || Parameters {
                        err: Error {
                            kind: ErrorKind::NotAJSONObject,
                            msg: "error".into(),
                        },
                    },
                    |rc| assert_eq!(rc, exitcode::USAGE),
                );
            }

            #[test]
            fn script_failed() {
                test(
                    || Parameters {
                        err: Error {
                            kind: ErrorKind::ScriptFailed {
                                rc: None,
                                stderr: "stderr".into(),
                                stdout: "stdout".into(),
                            },
                            msg: "error".into(),
                        },
                    },
                    |rc| assert_eq!(rc, exitcode::IOERR),
                );
            }

            #[test]
            fn template_not_found() {
                test(
                    || Parameters {
                        err: Error {
                            kind: ErrorKind::TemplateNotFound,
                            msg: "error".into(),
                        },
                    },
                    |rc| assert_eq!(rc, exitcode::DATAERR),
                );
            }

            #[test]
            fn unsupported_shell() {
                test(
                    || Parameters {
                        err: Error {
                            kind: ErrorKind::UnsupportedShell,
                            msg: "error".into(),
                        },
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

        mod to_string {
            use super::*;

            struct Context {
                msg: &'static str,
            }

            struct Parameters {
                err: Error,
            }

            #[test]
            fn git() {
                let err_code = git2::ErrorCode::Ambiguous;
                let err_class = git2::ErrorClass::Callback;
                let err_msg = "";
                test(
                    |ctx| Parameters {
                        err: Error {
                            kind: ErrorKind::Git(git2::Error::new(
                                git2::ErrorCode::Ambiguous,
                                git2::ErrorClass::Callback,
                                "",
                            )),
                            msg: ctx.msg.into(),
                        },
                    },
                    |ctx, str| {
                        let expected_str = format!(
                            "{}: {}",
                            ctx.msg,
                            git2::Error::new(err_code, err_class, err_msg)
                        );
                        assert_eq!(str, expected_str);
                    },
                );
            }

            #[test]
            fn home_dir_not_found() {
                test(
                    |ctx| Parameters {
                        err: Error {
                            kind: ErrorKind::HomeNotFound,
                            msg: ctx.msg.into(),
                        },
                    },
                    |ctx, str| assert_eq!(str, ctx.msg),
                );
            }

            #[test]
            fn io() {
                let kind = io::ErrorKind::PermissionDenied;
                test(
                    |ctx| Parameters {
                        err: Error {
                            kind: ErrorKind::IO(io::Error::from(kind)),
                            msg: ctx.msg.into(),
                        },
                    },
                    |ctx, str| {
                        let expected_str = format!("{}: {}", ctx.msg, io::Error::from(kind));
                        assert_eq!(str, expected_str);
                    },
                );
            }

            #[test]
            fn invalid_config() {
                test(
                    |ctx| Parameters {
                        err: Error {
                            kind: ErrorKind::InvalidConfig(vec![]),
                            msg: ctx.msg.into(),
                        },
                    },
                    |ctx, str| assert_eq!(str, ctx.msg),
                );
            }

            #[test]
            fn liquid() {
                let err_msg = "error";
                test(
                    |ctx| Parameters {
                        err: Error {
                            kind: ErrorKind::Liquid(liquid::Error::with_msg(err_msg)),
                            msg: ctx.msg.into(),
                        },
                    },
                    |ctx, str| {
                        let expected_str =
                            format!("{}: {}", ctx.msg, liquid::Error::with_msg(err_msg));
                        assert_eq!(str, expected_str);
                    },
                );
            }

            #[test]
            fn malformed_config() {
                let yaml = "{";
                test(
                    |ctx| Parameters {
                        err: Error {
                            kind: ErrorKind::MalformedConfig(
                                serde_yaml::from_str::<serde_yaml::Value>(yaml).unwrap_err(),
                            ),
                            msg: ctx.msg.into(),
                        },
                    },
                    |ctx, str| {
                        let expected_str = format!(
                            "{}: {}",
                            ctx.msg,
                            serde_yaml::from_str::<serde_yaml::Value>(yaml).unwrap_err()
                        );
                        assert_eq!(str, expected_str);
                    },
                );
            }

            #[test]
            fn malformed_values() {
                let json = "{";
                test(
                    |ctx| Parameters {
                        err: Error {
                            kind: ErrorKind::MalformedValues(
                                serde_json::from_str::<serde_json::Value>(json).unwrap_err(),
                            ),
                            msg: ctx.msg.into(),
                        },
                    },
                    |ctx, str| {
                        let expected_str = format!(
                            "{}: {}",
                            ctx.msg,
                            serde_json::from_str::<serde_json::Value>(json).unwrap_err()
                        );
                        assert_eq!(str, expected_str);
                    },
                )
            }

            #[test]
            fn not_a_json_object() {
                test(
                    |ctx| Parameters {
                        err: Error {
                            kind: ErrorKind::NotAJSONObject,
                            msg: ctx.msg.into(),
                        },
                    },
                    |ctx, str| assert_eq!(str, ctx.msg),
                );
            }

            #[test]
            fn script_failed() {
                test(
                    |ctx| Parameters {
                        err: Error {
                            kind: ErrorKind::ScriptFailed {
                                rc: None,
                                stderr: "stderr".into(),
                                stdout: "stdout".into(),
                            },
                            msg: ctx.msg.into(),
                        },
                    },
                    |ctx, str| assert_eq!(str, ctx.msg),
                );
            }

            #[test]
            fn template_not_found() {
                test(
                    |ctx| Parameters {
                        err: Error {
                            kind: ErrorKind::TemplateNotFound,
                            msg: ctx.msg.into(),
                        },
                    },
                    |ctx, str| assert_eq!(str, ctx.msg),
                );
            }

            #[test]
            fn unsupported_shell() {
                test(
                    |ctx| Parameters {
                        err: Error {
                            kind: ErrorKind::UnsupportedShell,
                            msg: ctx.msg.into(),
                        },
                    },
                    |ctx, str| assert_eq!(str, ctx.msg),
                );
            }

            fn test<P: Fn(&Context) -> Parameters, A: Fn(&Context, String)>(
                create_params_fn: P,
                assert_fn: A,
            ) {
                let ctx = Context { msg: "error" };
                let params = create_params_fn(&ctx);
                let str = params.err.to_string();
                assert_fn(&ctx, str);
            }
        }
    }
}
