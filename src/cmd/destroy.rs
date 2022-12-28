use std::fmt::{self, Debug, Formatter};

use log::info;

use crate::{
    cli::DestroyCommandArguments,
    err::Result,
    fs::{DefaultFileSystem, FileSystem},
    paths::{DefaultPaths, Paths},
};

pub struct DestroyCommand {
    args: DestroyCommandArguments,
    fs: Box<dyn FileSystem>,
    paths: Box<dyn Paths>,
}

impl DestroyCommand {
    pub fn new(args: DestroyCommandArguments) -> Self {
        Self {
            args,
            fs: Box::new(DefaultFileSystem),
            paths: Box::new(DefaultPaths::new()),
        }
    }

    pub fn run(self) -> Result<()> {
        info!("Destroying project");
        let project_dirpath = self.fs.canonicalize(&self.args.project_dirpath)?;
        self.fs.delete_dir(&project_dirpath)?;
        info!("Updating allowed directories list");
        let project_dirpath = project_dirpath.to_string_lossy().to_string();
        let allowed_dirs_filepath = self
            .paths
            .allowed_dirs(self.args.allowed_dirs_filepath, None)?;
        self.fs
            .ensure_lines_are_absent(&[&project_dirpath], &allowed_dirs_filepath, true)
    }
}

impl Debug for DestroyCommand {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        f.debug_struct("DestroyCommand")
            .field("args", &self.args)
            .finish()
    }
}

#[cfg(test)]
mod test {
    use std::{fs::remove_dir_all, path::PathBuf};

    use tempfile::tempdir;

    use crate::{err::Error, fs::StubFileSystem, paths::StubPaths};

    use super::*;

    mod destroy_command {
        use super::*;

        mod new {
            use super::*;

            #[test]
            fn cmd() {
                let args = DestroyCommandArguments::new(tempdir().unwrap().into_path());
                let cmd = DestroyCommand::new(args.clone());
                assert_eq!(cmd.args, args);
            }
        }

        mod run {
            use super::*;

            struct Context {
                allowed_dirs_filepath: PathBuf,
                project_dirpath: PathBuf,
            }

            struct Parameters {
                args: DestroyCommandArguments,
            }

            #[test]
            fn ok_when_default_args() {
                test(
                    |ctx| Parameters {
                        args: DestroyCommandArguments::new(ctx.project_dirpath.clone()),
                    },
                    assert,
                )
            }

            #[test]
            fn ok_when_custom_args() {
                let allowed_dirs_filepath = tempdir().unwrap().into_path().join("allowed-dirs");
                test(
                    |ctx| Parameters {
                        args: DestroyCommandArguments {
                            allowed_dirs_filepath: Some(allowed_dirs_filepath.clone()),
                            project_dirpath: ctx.project_dirpath.clone(),
                        },
                    },
                    assert,
                )
            }

            fn assert(ctx: &Context, res: Result<()>) {
                res.unwrap();
                assert!(!ctx.project_dirpath.exists());
            }

            fn test<P: Fn(&Context) -> Parameters, A: Fn(&Context, Result<()>)>(
                create_params_fn: P,
                assert_fn: A,
            ) {
                let ctx = Context {
                    allowed_dirs_filepath: tempdir().unwrap().into_path().join("allowed-dirs"),
                    project_dirpath: tempdir().unwrap().into_path().canonicalize().unwrap(),
                };
                let params = create_params_fn(&ctx);
                let fs = StubFileSystem::new()
                    .with_stub_of_canonicalize({
                        let project_dirpath = params.args.project_dirpath.clone();
                        move |_, path| {
                            assert_eq!(path, project_dirpath);
                            path.canonicalize().map_err(Error::IO)
                        }
                    })
                    .with_stub_of_delete_dir({
                        let project_dirpath = params.args.project_dirpath.clone();
                        move |_, path| {
                            assert_eq!(path, project_dirpath);
                            remove_dir_all(path).map_err(Error::IO)
                        }
                    })
                    .with_stub_of_ensure_lines_are_absent({
                        let allowed_dirs_filepath = ctx.allowed_dirs_filepath.clone();
                        let project_dirpath = params.args.project_dirpath.clone();
                        move |_, lines, path, lock| {
                            assert_eq!(lines, vec![project_dirpath.to_string_lossy().to_string()]);
                            assert_eq!(path, allowed_dirs_filepath);
                            assert!(lock);
                            Ok(())
                        }
                    });
                let paths = StubPaths::new().with_stub_of_allowed_dirs({
                    let filepath = ctx.allowed_dirs_filepath.clone();
                    let expected_allowed_dirs_filepath = params.args.allowed_dirs_filepath.clone();
                    move |_, allowed_dirs_filepath, project_dirpath| {
                        assert_eq!(allowed_dirs_filepath, expected_allowed_dirs_filepath);
                        assert!(project_dirpath.is_none());
                        Ok(filepath.clone())
                    }
                });
                let cmd = DestroyCommand {
                    args: params.args,
                    fs: Box::new(fs),
                    paths: Box::new(paths),
                };
                let res = cmd.run();
                assert_fn(&ctx, res);
            }
        }
    }
}
