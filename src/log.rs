use std::{
    io::{stderr, Stderr, Write},
    sync::Mutex,
};

use log::{
    set_boxed_logger, set_max_level, Level, LevelFilter, Log, Metadata, Record, SetLoggerError,
};

const DEBUG_COLOR: (u8, u8, u8) = (193, 193, 193);
const ERROR_COLOR: (u8, u8, u8) = (255, 0, 0);
const INFO_COLOR: (u8, u8, u8) = (61, 124, 240);
const TRACE_COLOR: (u8, u8, u8) = (61, 61, 61);
const WARN_COLOR: (u8, u8, u8) = (245, 114, 0);

pub struct Logger<W: Write + Send> {
    lvl: LevelFilter,
    out: Mutex<W>,
    with_color: bool,
}

impl Logger<Stderr> {
    pub fn init(lvl: LevelFilter, with_color: bool) -> Result<(), SetLoggerError> {
        let logger = Self {
            lvl,
            out: Mutex::new(stderr()),
            with_color,
        };
        set_max_level(lvl);
        set_boxed_logger(Box::new(logger))
    }
}

impl<W: Write + Send> Log for Logger<W> {
    fn enabled(&self, metadata: &Metadata) -> bool {
        metadata.level() <= self.lvl
    }

    fn flush(&self) {}

    fn log(&self, record: &Record) {
        if self.enabled(record.metadata()) {
            let (lvl, color) = match record.level() {
                Level::Debug => ("DEBUG", DEBUG_COLOR),
                Level::Error => ("ERROR", ERROR_COLOR),
                Level::Info => ("INFO", INFO_COLOR),
                Level::Trace => ("TRACE", TRACE_COLOR),
                Level::Warn => ("WARN", WARN_COLOR),
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
    use super::*;

    mod logger {
        use super::*;

        mod enabled {
            use super::*;

            macro_rules! test {
                ($ident:ident, $lvl:expr, $max_lvl:expr, $enabled:literal) => {
                    #[test]
                    fn $ident() {
                        let metadata = Metadata::builder().level($lvl).build();
                        let logger = Logger {
                            lvl: $max_lvl,
                            out: Mutex::new(vec![]),
                            with_color: false,
                        };
                        assert_eq!(logger.enabled(&metadata), $enabled);
                    }
                };
            }

            test!(
                false_if_max_lvl_is_off,
                Level::Error,
                LevelFilter::Off,
                false
            );
            test!(
                false_if_max_lvl_is_lower_than_lvl,
                Level::Warn,
                LevelFilter::Error,
                false
            );
            test!(
                true_if_max_lvl_is_equal_to_lvl,
                Level::Error,
                LevelFilter::Error,
                true
            );
            test!(
                true_if_max_lvl_is_greater_than_lvl,
                Level::Error,
                LevelFilter::Warn,
                true
            );
        }

        mod log {
            use super::*;

            struct Context<'a> {
                debug1_msg: &'a str,
                debug2_msg: &'a str,
                error1_msg: &'a str,
                error2_msg: &'a str,
                info1_msg: &'a str,
                info2_msg: &'a str,
                trace1_msg: &'a str,
                trace2_msg: &'a str,
                warn1_msg: &'a str,
                warn2_msg: &'a str,
            }

            struct Logs<'a> {
                debug1: &'a str,
                debug2: &'a str,
                error1: &'a str,
                error2: &'a str,
                info1: &'a str,
                info2: &'a str,
                trace1: &'a str,
                trace2: &'a str,
                warn1: &'a str,
                warn2: &'a str,
            }

            #[test]
            fn with_color() {
                test(true, |ctx, logs| {
                    assert_eq!(
                        logs.error1,
                        format!(
                            "\x1b[38;2;{};{};{}m[ERROR] {}\x1b[0m",
                            ERROR_COLOR.0, ERROR_COLOR.1, ERROR_COLOR.2, ctx.error1_msg
                        )
                    );
                    assert_eq!(
                        logs.error2,
                        format!(
                            "\x1b[38;2;{};{};{}m[ERROR] {}\x1b[0m",
                            ERROR_COLOR.0, ERROR_COLOR.1, ERROR_COLOR.2, ctx.error2_msg
                        )
                    );
                    assert_eq!(
                        logs.warn1,
                        format!(
                            "\x1b[38;2;{};{};{}m [WARN] {}\x1b[0m",
                            WARN_COLOR.0, WARN_COLOR.1, WARN_COLOR.2, ctx.warn1_msg
                        )
                    );
                    assert_eq!(
                        logs.warn2,
                        format!(
                            "\x1b[38;2;{};{};{}m [WARN] {}\x1b[0m",
                            WARN_COLOR.0, WARN_COLOR.1, WARN_COLOR.2, ctx.warn2_msg
                        )
                    );
                    assert_eq!(
                        logs.info1,
                        format!(
                            "\x1b[38;2;{};{};{}m [INFO] {}\x1b[0m",
                            INFO_COLOR.0, INFO_COLOR.1, INFO_COLOR.2, ctx.info1_msg
                        )
                    );
                    assert_eq!(
                        logs.info2,
                        format!(
                            "\x1b[38;2;{};{};{}m [INFO] {}\x1b[0m",
                            INFO_COLOR.0, INFO_COLOR.1, INFO_COLOR.2, ctx.info2_msg
                        )
                    );
                    assert_eq!(
                        logs.debug1,
                        format!(
                            "\x1b[38;2;{};{};{}m[DEBUG] {}\x1b[0m",
                            DEBUG_COLOR.0, DEBUG_COLOR.1, DEBUG_COLOR.2, ctx.debug1_msg
                        )
                    );
                    assert_eq!(
                        logs.debug2,
                        format!(
                            "\x1b[38;2;{};{};{}m[DEBUG] {}\x1b[0m",
                            DEBUG_COLOR.0, DEBUG_COLOR.1, DEBUG_COLOR.2, ctx.debug2_msg
                        )
                    );
                    assert_eq!(
                        logs.trace1,
                        format!(
                            "\x1b[38;2;{};{};{}m[TRACE] {}\x1b[0m",
                            TRACE_COLOR.0, TRACE_COLOR.1, TRACE_COLOR.2, ctx.trace1_msg
                        )
                    );
                    assert_eq!(
                        logs.trace2,
                        format!(
                            "\x1b[38;2;{};{};{}m[TRACE] {}\x1b[0m",
                            TRACE_COLOR.0, TRACE_COLOR.1, TRACE_COLOR.2, ctx.trace2_msg
                        )
                    );
                })
            }

            #[test]
            fn without_color() {
                test(false, |ctx, logs| {
                    assert_eq!(logs.error1, format!("[ERROR] {}", ctx.error1_msg));
                    assert_eq!(logs.error2, format!("[ERROR] {}", ctx.error2_msg));
                    assert_eq!(logs.warn1, format!(" [WARN] {}", ctx.warn1_msg));
                    assert_eq!(logs.warn2, format!(" [WARN] {}", ctx.warn2_msg));
                    assert_eq!(logs.info1, format!(" [INFO] {}", ctx.info1_msg));
                    assert_eq!(logs.info2, format!(" [INFO] {}", ctx.info2_msg));
                    assert_eq!(logs.debug1, format!("[DEBUG] {}", ctx.debug1_msg));
                    assert_eq!(logs.debug2, format!("[DEBUG] {}", ctx.debug2_msg));
                    assert_eq!(logs.trace1, format!("[TRACE] {}", ctx.trace1_msg));
                    assert_eq!(logs.trace2, format!("[TRACE] {}", ctx.trace2_msg));
                })
            }

            #[inline]
            fn test<A: Fn(&Context, &Logs)>(with_color: bool, assert_fn: A) {
                let error1_msg = "error1";
                let error2_msg = "error2";
                let error_record = Record::builder()
                    .level(Level::Error)
                    .args(format_args!("error1\nerror2"))
                    .build();
                let warn1_msg = "warn1";
                let warn2_msg = "warn2";
                let warn_record = Record::builder()
                    .level(Level::Warn)
                    .args(format_args!("warn1\nwarn2"))
                    .build();
                let info1_msg = "info1";
                let info2_msg = "info2";
                let info_record = Record::builder()
                    .level(Level::Info)
                    .args(format_args!("info1\ninfo2"))
                    .build();
                let debug1_msg = "debug1";
                let debug2_msg = "debug2";
                let debug_record = Record::builder()
                    .level(Level::Debug)
                    .args(format_args!("debug1\ndebug2"))
                    .build();
                let trace1_msg = "trace1";
                let trace2_msg = "trace2";
                let trace_record = Record::builder()
                    .level(Level::Trace)
                    .args(format_args!("trace1\ntrace2"))
                    .build();
                let logger = Logger {
                    lvl: LevelFilter::Trace,
                    out: Mutex::new(vec![]),
                    with_color,
                };
                logger.log(&error_record);
                logger.log(&warn_record);
                logger.log(&info_record);
                logger.log(&debug_record);
                logger.log(&trace_record);
                let out = logger.out.into_inner().unwrap();
                let logs = String::from_utf8(out).unwrap();
                let ctx = Context {
                    debug1_msg,
                    debug2_msg,
                    error1_msg,
                    error2_msg,
                    info1_msg,
                    info2_msg,
                    trace1_msg,
                    trace2_msg,
                    warn1_msg,
                    warn2_msg,
                };
                let lines: Vec<&str> = logs.lines().collect();
                let logs = Logs {
                    debug1: lines[6],
                    debug2: lines[7],
                    error1: lines[0],
                    error2: lines[1],
                    info1: lines[4],
                    info2: lines[5],
                    trace1: lines[8],
                    trace2: lines[9],
                    warn1: lines[2],
                    warn2: lines[3],
                };
                assert_fn(&ctx, &logs);
            }
        }
    }
}
