use std::{
    io::{stderr, Stderr, Write},
    sync::Mutex,
};

use log::{
    set_boxed_logger, set_max_level, Level, LevelFilter, Log, Metadata, Record, SetLoggerError,
};

const DEBUG_LABEL: &str = "DEBUG";
const ERROR_LABEL: &str = "ERROR";
const INFO_LABEL: &str = "INFO";
const TRACE_LABEL: &str = "TRACE";
const WARN_LABEL: &str = "WARN";

const DEBUG_COLOR: (u8, u8, u8) = (193, 193, 193);
const ERROR_COLOR: (u8, u8, u8) = (255, 0, 0);
const INFO_COLOR: (u8, u8, u8) = (61, 124, 240);
const TRACE_COLOR: (u8, u8, u8) = (61, 61, 61);
const WARN_COLOR: (u8, u8, u8) = (245, 114, 0);

pub struct Logger<W: Write + Send> {
    filter: LevelFilter,
    out: Mutex<W>,
    with_color: bool,
}

impl Logger<Stderr> {
    pub fn init(filter: LevelFilter, with_color: bool) -> Result<(), SetLoggerError> {
        let logger = Self {
            filter,
            out: Mutex::new(stderr()),
            with_color,
        };
        set_max_level(filter);
        set_boxed_logger(Box::new(logger))
    }
}

impl<W: Write + Send> Log for Logger<W> {
    fn enabled(&self, metadata: &Metadata) -> bool {
        metadata.level() <= self.filter
    }

    fn flush(&self) {}

    fn log(&self, record: &Record) {
        if self.enabled(record.metadata()) {
            let (lvl, color) = match record.level() {
                Level::Debug => (DEBUG_LABEL, DEBUG_COLOR),
                Level::Error => (ERROR_LABEL, ERROR_COLOR),
                Level::Info => (INFO_LABEL, INFO_COLOR),
                Level::Trace => (TRACE_LABEL, TRACE_COLOR),
                Level::Warn => (WARN_LABEL, WARN_COLOR),
            };
            let msg = record.args().to_string();
            for line in msg.lines() {
                let log = format!("{:>7} {}", format!("[{}]", lvl), line);
                let mut out = self.out.lock().unwrap();
                if self.with_color {
                    writeln!(
                        out,
                        "\x1b[38;2;{};{};{}m{}\x1b[0m",
                        color.0, color.1, color.2, log
                    )
                    .unwrap();
                } else {
                    writeln!(out, "{}", log).unwrap();
                }
            }
        }
    }
}

#[cfg(test)]
mod test {
    use std::fmt::Arguments;

    use super::*;

    mod logger {
        use super::*;

        mod enabled {
            use super::*;

            struct Parameters {
                filter: LevelFilter,
                lvl: Level,
            }

            #[test]
            fn false_when_filter_is_off() {
                test(
                    || Parameters {
                        lvl: Level::Error,
                        filter: LevelFilter::Off,
                    },
                    |enabled| assert!(!enabled),
                )
            }

            #[test]
            fn false_when_filter_is_lower_than_lvl() {
                test(
                    || Parameters {
                        lvl: Level::Warn,
                        filter: LevelFilter::Error,
                    },
                    |enabled| assert!(!enabled),
                )
            }

            #[test]
            fn true_when_filter_is_equal_to_lvl() {
                test(
                    || Parameters {
                        lvl: Level::Error,
                        filter: LevelFilter::Error,
                    },
                    |enabled| assert!(enabled),
                )
            }

            #[test]
            fn true_when_filter_is_greater_than_lvl() {
                test(
                    || Parameters {
                        lvl: Level::Error,
                        filter: LevelFilter::Warn,
                    },
                    |enabled| assert!(enabled),
                )
            }

            fn test<P: Fn() -> Parameters, A: Fn(bool)>(create_params_fn: P, assert_fn: A) {
                let params = create_params_fn();
                let metadata = Metadata::builder().level(params.lvl).build();
                let logger = Logger {
                    filter: params.filter,
                    out: Mutex::new(vec![]),
                    with_color: false,
                };
                let enabled = logger.enabled(&metadata);
                assert_fn(enabled);
            }
        }

        mod log {
            use super::*;

            struct Context {
                args: Arguments<'static>,
            }

            struct Parameters {
                lvl: Level,
                with_color: bool,
            }

            #[test]
            fn debug_with_color() {
                test(
                    |_| Parameters {
                        lvl: Level::Debug,
                        with_color: true,
                    },
                    |ctx, out| assert_logs(ctx, out, DEBUG_LABEL, Some(DEBUG_COLOR)),
                );
            }

            #[test]
            fn debug_without_color() {
                test(
                    |_| Parameters {
                        lvl: Level::Debug,
                        with_color: false,
                    },
                    |ctx, out| assert_logs(ctx, out, DEBUG_LABEL, None),
                );
            }

            #[test]
            fn error_with_color() {
                test(
                    |_| Parameters {
                        lvl: Level::Error,
                        with_color: true,
                    },
                    |ctx, out| assert_logs(ctx, out, ERROR_LABEL, Some(ERROR_COLOR)),
                );
            }

            #[test]
            fn error_without_color() {
                test(
                    |_| Parameters {
                        lvl: Level::Error,
                        with_color: false,
                    },
                    |ctx, out| assert_logs(ctx, out, ERROR_LABEL, None),
                );
            }

            #[test]
            fn info_with_color() {
                test(
                    |_| Parameters {
                        lvl: Level::Info,
                        with_color: true,
                    },
                    |ctx, out| assert_logs(ctx, out, INFO_LABEL, Some(INFO_COLOR)),
                );
            }

            #[test]
            fn info_without_color() {
                test(
                    |_| Parameters {
                        lvl: Level::Info,
                        with_color: false,
                    },
                    |ctx, out| assert_logs(ctx, out, INFO_LABEL, None),
                );
            }

            #[test]
            fn trace_with_color() {
                test(
                    |_| Parameters {
                        lvl: Level::Trace,
                        with_color: true,
                    },
                    |ctx, out| assert_logs(ctx, out, TRACE_LABEL, Some(TRACE_COLOR)),
                );
            }

            #[test]
            fn trace_without_color() {
                test(
                    |_| Parameters {
                        lvl: Level::Trace,
                        with_color: false,
                    },
                    |ctx, out| assert_logs(ctx, out, TRACE_LABEL, None),
                );
            }

            #[test]
            fn warn_with_color() {
                test(
                    |_| Parameters {
                        lvl: Level::Warn,
                        with_color: true,
                    },
                    |ctx, out| assert_logs(ctx, out, WARN_LABEL, Some(WARN_COLOR)),
                );
            }

            #[test]
            fn warn_without_color() {
                test(
                    |_| Parameters {
                        lvl: Level::Warn,
                        with_color: false,
                    },
                    |ctx, out| assert_logs(ctx, out, WARN_LABEL, None),
                );
            }

            fn assert_logs(ctx: &Context, out: String, label: &str, color: Option<(u8, u8, u8)>) {
                let msg = format!("{}", ctx.args);
                let lines: Vec<&str> = out.lines().collect();
                let log_fn = |line: &str| format!("{:>7} {}", format!("[{}]", label), line);
                let expected_lines: Vec<String> = if let Some(color) = color {
                    msg.lines()
                        .map(|line| {
                            format!(
                                "\x1b[38;2;{};{};{}m{}\x1b[0m",
                                color.0,
                                color.1,
                                color.2,
                                log_fn(line)
                            )
                        })
                        .collect()
                } else {
                    msg.lines().map(|line| log_fn(line)).collect()
                };
                assert_eq!(lines, expected_lines);
            }

            fn test<P: Fn(&Context) -> Parameters, A: Fn(&Context, String)>(
                create_params_fn: P,
                assert_fn: A,
            ) {
                let ctx = Context {
                    args: format_args!("line1\nline2"),
                };
                let params = create_params_fn(&ctx);
                let record = Record::builder().level(params.lvl).args(ctx.args).build();
                let logger = Logger {
                    filter: LevelFilter::Trace,
                    out: Mutex::new(vec![]),
                    with_color: params.with_color,
                };
                logger.log(&record);
                let out = logger.out.into_inner().unwrap();
                let out = String::from_utf8(out).unwrap();
                assert_fn(&ctx, out);
            }
        }
    }
}
