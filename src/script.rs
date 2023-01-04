use std::process::Command;

use log::debug;

use crate::err::{Error, ErrorKind, Result};

#[cfg_attr(test, stub_trait::stub)]
pub trait ScriptRunner {
    fn run(&self, shell: &str, script: &str) -> Result<String>;
}

pub struct DefaultScriptRunner;

impl DefaultScriptRunner {
    #[inline]
    fn output_to_string(out: Vec<u8>) -> String {
        String::from_utf8_lossy(&out).trim().to_string()
    }
}

impl ScriptRunner for DefaultScriptRunner {
    fn run(&self, shell: &str, script: &str) -> Result<String> {
        debug!("Executing `{} -c {}`", shell, script);
        let output = Command::new(shell)
            .args(["-c", script])
            .output()
            .map_err(|err| Error {
                kind: ErrorKind::IO(err),
                msg: format!("Unable to execute {}", script),
            })?;
        let stdout = Self::output_to_string(output.stdout);
        if output.status.success() {
            Ok(stdout)
        } else {
            let stderr = Self::output_to_string(output.stderr);
            Err(Error {
                kind: ErrorKind::ScriptFailed {
                    rc: output.status.code(),
                    stderr,
                    stdout,
                },
                msg: format!("{} failed", shell),
            })
        }
    }
}

#[cfg(test)]
mod test {
    use super::*;

    mod default_script_runner {
        use super::*;

        mod run {
            use super::*;

            struct Context {
                shell: &'static str,
            }

            struct Parameters {
                script: String,
            }

            #[test]
            fn err_when_status_is_not_ok() {
                test(
                    |_| Parameters {
                        script: "echo -n toto | grep tata".into(),
                    },
                    |_, res| {
                        res.unwrap_err();
                    },
                )
            }

            #[test]
            fn ok_when_status_is_ok() {
                let stdout = "toto";
                test(
                    |_| Parameters {
                        script: format!("echo toto | grep {}", stdout),
                    },
                    |_, res| {
                        assert_eq!(res.unwrap(), stdout);
                    },
                )
            }

            fn test<P: Fn(&Context) -> Parameters, A: Fn(&Context, Result<String>)>(
                create_params_fn: P,
                assert_fn: A,
            ) {
                let ctx = Context { shell: "/bin/bash" };
                let params = create_params_fn(&ctx);
                let res = DefaultScriptRunner.run(ctx.shell, &params.script);
                assert_fn(&ctx, res);
            }
        }
    }
}
