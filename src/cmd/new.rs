use std::{
    env::current_dir,
    fmt::{self, Debug, Formatter},
    fs::{create_dir_all, remove_dir_all},
    io,
    path::{Path, PathBuf},
};

use log::debug;
use tempfile::tempdir;

use crate::{
    cli::NewCommandArguments,
    err::Error,
    git::{DefaultGit, Git, Reference},
};

use super::{Command, CommandKind, Result};

type CWDFn = dyn Fn() -> io::Result<PathBuf>;
type TempdirFn = dyn Fn() -> io::Result<PathBuf>;

pub struct NewCommand {
    args: NewCommandArguments,
    cwd_fn: Box<CWDFn>,
    git: Box<dyn Git>,
    tempdir_fn: Box<TempdirFn>,
}

impl NewCommand {
    pub fn new(args: NewCommandArguments) -> Self {
        Self {
            args,
            cwd_fn: Box::new(current_dir),
            git: Box::new(DefaultGit),
            tempdir_fn: Box::new(|| tempdir().map(|tempdir| tempdir.into_path())),
        }
    }

    #[inline]
    fn delete_dir(path: &Path) {
        if let Err(err) = remove_dir_all(path) {
            debug!("Unable to delete {}: {}", path.display(), err);
        }
    }

    fn render_files_recursively(_tpl_dirpath: &Path, _dest: &Path) -> Result {
        Ok(())
    }
}

impl Command for NewCommand {
    fn kind(self) -> CommandKind {
        CommandKind::New(self)
    }

    fn run(self) -> Result {
        let dest = self
            .args
            .dest
            .map(|dest| {
                debug!("Using {} as destination directory", dest.display());
                Ok(dest)
            })
            .unwrap_or_else(|| {
                debug!("No destination directory set, using current working directory as parent");
                (self.cwd_fn)().map(|cwd| cwd.join(&self.args.name))
            })
            .map_err(Error::IO)?;
        if dest.exists() {
            return Err(Error::DestinationDirectoryAlreadyExists(dest));
        }
        debug!("Creating {} directory", dest.display());
        create_dir_all(&dest).map_err(Error::IO)?;
        debug!("Creating temporary directory");
        let tpl_repo_path = match (self.tempdir_fn)() {
            Ok(path) => path,
            Err(err) => {
                Self::delete_dir(&dest);
                return Err(Error::IO(err));
            }
        };
        let git_ref = self
            .args
            .git_branch
            .map(Reference::Branch)
            .or_else(|| self.args.git_tag.map(Reference::Tag));
        let res = self
            .git
            .checkout_repository(&self.args.git, git_ref, &tpl_repo_path)
            .map_err(Error::Git)
            .and_then(|_| Self::render_files_recursively(&tpl_repo_path, &dest));
        Self::delete_dir(&tpl_repo_path);
        if res.is_err() {
            Self::delete_dir(&dest);
        }
        res
    }
}

impl Debug for NewCommand {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        f.debug_struct("NewCommand")
            .field("args", &self.args)
            .finish()
    }
}

#[cfg(test)]
mod test {
    use std::fs::File;

    use git2::Repository;

    use crate::{cli::DEFAULT_TPL_GIT_REPO_URL, git::StubGit};

    use super::*;

    mod new_command {
        use super::*;

        mod new {
            use super::*;

            #[test]
            fn cmd() {
                let args = NewCommandArguments::default_for_test();
                let cmd = NewCommand::new(args.clone());
                assert_eq!(cmd.args, args);
            }
        }

        mod kind {
            use super::*;

            #[test]
            fn new() {
                let cmd = NewCommand {
                    args: NewCommandArguments::default_for_test(),
                    cwd_fn: Box::new(current_dir),
                    git: Box::new(StubGit::new()),
                    tempdir_fn: Box::new(|| tempdir().map(|tempdir| tempdir.into_path())),
                };
                match cmd.kind() {
                    CommandKind::New(_) => (),
                }
            }
        }

        mod run {
            use super::*;

            type CheckoutRepositoryFn =
                dyn Fn(Repository) -> std::result::Result<Repository, git2::Error>;
            type CWDFn = dyn Fn(PathBuf) -> io::Result<PathBuf>;
            type TempdirFn = dyn Fn(PathBuf) -> io::Result<PathBuf>;

            struct Context<'a> {
                cwd: &'a Path,
                tpl_repo_path: &'a Path,
            }

            struct Data {
                dest: PathBuf,
                params: Parameters,
            }

            struct Parameters {
                args: NewCommandArguments,
                checkout_repo_fn: Box<CheckoutRepositoryFn>,
                cwd_fn: Box<CWDFn>,
                git_ref: Option<Reference>,
                tempdir_fn: Box<TempdirFn>,
            }

            #[test]
            fn dest_dir_already_exists_err() {
                test(
                    move |ctx| {
                        let name = "test";
                        let dest = ctx.cwd.join(name);
                        File::create(&dest).unwrap();
                        Data {
                            params: Parameters {
                                args: NewCommandArguments {
                                    name: name.into(),
                                    ..NewCommandArguments::default_for_test()
                                },
                                checkout_repo_fn: Box::new(Ok),
                                cwd_fn: Box::new(Ok),
                                git_ref: None,
                                tempdir_fn: Box::new(Ok),
                            },
                            dest,
                        }
                    },
                    |_, dest, res| match res.unwrap_err() {
                        Error::DestinationDirectoryAlreadyExists(path) => assert_eq!(path, dest),
                        err => {
                            let expected_err =
                                Error::DestinationDirectoryAlreadyExists(dest.to_path_buf());
                            panic!("expected {:?} (actual: {:?})", expected_err, err);
                        }
                    },
                );
            }

            #[test]
            fn err_if_tempdir_failed() {
                let err_kind = io::ErrorKind::PermissionDenied;
                test(
                    move |ctx| {
                        let name = "test";
                        Data {
                            params: Parameters {
                                args: NewCommandArguments {
                                    name: name.into(),
                                    ..NewCommandArguments::default_for_test()
                                },
                                checkout_repo_fn: Box::new(Ok),
                                cwd_fn: Box::new(Ok),
                                git_ref: None,
                                tempdir_fn: Box::new(move |_| Err(io::Error::from(err_kind))),
                            },
                            dest: ctx.cwd.join(name),
                        }
                    },
                    |_, dest, res| {
                        match res.unwrap_err() {
                            Error::IO(_) => (),
                            err => {
                                let expected_err = Error::IO(io::Error::from(err_kind));
                                panic!("expected {:?} (actual: {:?})", expected_err, err);
                            }
                        }
                        assert!(!dest.is_dir());
                    },
                );
            }

            #[test]
            fn err_if_checkout_failed() {
                let err_code = git2::ErrorCode::Ambiguous;
                let err_class = git2::ErrorClass::Callback;
                let err_msg = "error";
                test(
                    move |ctx| {
                        let name = "test";
                        Data {
                            params: Parameters {
                                args: NewCommandArguments {
                                    name: name.into(),
                                    ..NewCommandArguments::default_for_test()
                                },
                                checkout_repo_fn: Box::new(move |_| {
                                    Err(git2::Error::new(err_code, err_class, err_msg))
                                }),
                                cwd_fn: Box::new(Ok),
                                git_ref: None,
                                tempdir_fn: Box::new(Ok),
                            },
                            dest: ctx.cwd.join(name),
                        }
                    },
                    |ctx, dest, res| {
                        match res.unwrap_err() {
                            Error::Git(_) => (),
                            err => {
                                let expected_err = git2::Error::new(err_code, err_class, err_msg);
                                panic!("expected {:?} (actual: {:?})", expected_err, err);
                            }
                        }
                        assert!(!ctx.tpl_repo_path.is_dir());
                        assert!(!dest.is_dir());
                    },
                );
            }

            #[test]
            fn ok_when_default_args() {
                ok(move |ctx| {
                    let name = "test";
                    Data {
                        params: Parameters {
                            args: NewCommandArguments {
                                name: name.into(),
                                ..NewCommandArguments::default_for_test()
                            },
                            checkout_repo_fn: Box::new(Ok),
                            cwd_fn: Box::new(Ok),
                            git_ref: None,
                            tempdir_fn: Box::new(Ok),
                        },
                        dest: ctx.cwd.join(name),
                    }
                })
            }

            #[test]
            fn ok_when_custom_args() {
                ok(move |_| {
                    let dest = tempdir().unwrap().into_path().join("test");
                    let branch = "develop";
                    Data {
                        params: Parameters {
                            args: NewCommandArguments {
                                dest: Some(dest.clone()),
                                git: format!("{}2", DEFAULT_TPL_GIT_REPO_URL),
                                git_branch: Some(branch.into()),
                                git_tag: None,
                                name: "test".into(),
                            },
                            checkout_repo_fn: Box::new(Ok),
                            cwd_fn: Box::new(Ok),
                            git_ref: Some(Reference::Branch(branch.into())),
                            tempdir_fn: Box::new(Ok),
                        },
                        dest,
                    }
                })
            }

            #[test]
            fn ok_when_custom_tag() {
                ok(move |ctx| {
                    let name = "test";
                    let tag = "v1.0.0";
                    Data {
                        params: Parameters {
                            args: NewCommandArguments {
                                git_tag: Some(tag.into()),
                                name: name.into(),
                                ..NewCommandArguments::default_for_test()
                            },
                            checkout_repo_fn: Box::new(Ok),
                            cwd_fn: Box::new(Ok),
                            git_ref: Some(Reference::Tag(tag.into())),
                            tempdir_fn: Box::new(Ok),
                        },
                        dest: ctx.cwd.join(name),
                    }
                })
            }

            #[inline]
            fn ok<D: Fn(&Context) -> Data>(data_from_fn: D) {
                test(data_from_fn, |ctx, dest, res| {
                    res.unwrap();
                    assert!(!ctx.tpl_repo_path.is_dir());
                    assert!(dest.is_dir());
                });
            }

            #[inline]
            fn test<D: Fn(&Context) -> Data, A: Fn(&Context, &Path, Result)>(
                data_from_fn: D,
                assert_fn: A,
            ) {
                let cwd = tempdir().unwrap().into_path();
                let tpl_repo_path = tempdir().unwrap().into_path();
                let ctx = Context {
                    cwd: &cwd,
                    tpl_repo_path: &tpl_repo_path,
                };
                let data = data_from_fn(&ctx);
                let git = StubGit::new().with_stub_of_checkout_repository({
                    let expected_url = data.params.args.git.clone();
                    let tpl_repo_path = tpl_repo_path.clone();
                    move |_, url, reference, dest| {
                        assert_eq!(url, expected_url);
                        assert_eq!(reference, data.params.git_ref);
                        assert_eq!(dest, tpl_repo_path);
                        (data.params.checkout_repo_fn)(Repository::init(&tpl_repo_path).unwrap())
                    }
                });
                let cmd = NewCommand {
                    args: data.params.args,
                    cwd_fn: Box::new({
                        let cwd = cwd.clone();
                        move || (data.params.cwd_fn)(cwd.clone())
                    }),
                    git: Box::new(git),
                    tempdir_fn: Box::new({
                        let tpl_repo_path = tpl_repo_path.clone();
                        move || (data.params.tempdir_fn)(tpl_repo_path.clone())
                    }),
                };
                assert_fn(&ctx, &data.dest, cmd.run());
            }
        }
    }
}
