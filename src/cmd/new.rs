use std::{
    fmt::{self, Debug, Formatter},
    path::Path,
};

use log::{debug, info, warn};

use crate::{
    cli::NewCommandArguments,
    err::Error,
    fs::{DefaultFileSystem, FileSystem},
    git::{DefaultGit, Git, Reference},
    renderer::{LiquidRenderer, Renderer},
};

use super::{Command, CommandKind, Result};

const NAME_VAR_NAME: &str = "name";

pub struct NewCommand {
    args: NewCommandArguments,
    fs: Box<dyn FileSystem>,
    git: Box<dyn Git>,
    renderer: Box<dyn Renderer>,
}

impl NewCommand {
    pub fn new(args: NewCommandArguments) -> Self {
        Self {
            args,
            fs: Box::new(DefaultFileSystem),
            git: Box::new(DefaultGit),
            renderer: Box::new(LiquidRenderer::new()),
        }
    }

    #[inline]
    fn delete_dir(path: &Path, fs: &dyn FileSystem) {
        if let Err(err) = fs.delete_dir(path) {
            warn!("Unable to delete {}: {}", path.display(), err);
        }
    }
}

impl Command for NewCommand {
    fn kind(self) -> CommandKind {
        CommandKind::New(self)
    }

    fn run(self) -> Result {
        info!("Creating project `{}`", self.args.name);
        let dest = self
            .args
            .dest
            .map(|dest| {
                debug!("Using {} as destination directory", dest.display());
                Ok(dest)
            })
            .unwrap_or_else(|| {
                debug!("No destination directory set, using current working directory as parent");
                self.fs.cwd().map(|cwd| cwd.join(&self.args.name))
            })?;
        if dest.exists() {
            return Err(Error::DestinationDirectoryAlreadyExists(dest));
        }
        self.fs.create_dir(&dest)?;
        let tpl_repo_path = match self.fs.create_temp_dir() {
            Ok(path) => path,
            Err(err) => {
                Self::delete_dir(&dest, self.fs.as_ref());
                return Err(err);
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
            .and_then(|_| {
                let tpl_dirpath = tpl_repo_path.join(&self.args.tpl);
                if tpl_dirpath.is_dir() {
                    let mut vars = self.args.vars;
                    vars.push((NAME_VAR_NAME.into(), self.args.name));
                    self.renderer.render_recursively(&tpl_dirpath, &dest, vars)
                } else {
                    Err(Error::TemplateNotFound(self.args.tpl))
                }
            });
        Self::delete_dir(&tpl_repo_path, self.fs.as_ref());
        if res.is_err() {
            Self::delete_dir(&dest, self.fs.as_ref());
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
    use std::{
        fs::{create_dir_all, remove_dir_all, File},
        io::{self},
        path::PathBuf,
    };

    use git2::Repository;
    use tempfile::tempdir;

    use crate::{
        cli::DEFAULT_TPL_GIT_REPO_URL, fs::StubFileSystem, git::StubGit, renderer::StubRenderer,
    };

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
                    fs: Box::new(StubFileSystem::new()),
                    git: Box::new(StubGit::new()),
                    renderer: Box::new(StubRenderer::new()),
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
            type CWDFn = dyn Fn() -> io::Result<()>;
            type RenderRecursivelyFn = dyn Fn() -> Result;
            type TempdirFn = dyn Fn() -> io::Result<()>;

            struct Context<'a> {
                cwd: &'a Path,
                tpl: &'a str,
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
                render_recursively_fn: Box<RenderRecursivelyFn>,
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
                                    tpl: ctx.tpl.into(),
                                    ..NewCommandArguments::default_for_test()
                                },
                                checkout_repo_fn: Box::new(Ok),
                                cwd_fn: Box::new(|| Ok(())),
                                git_ref: None,
                                render_recursively_fn: Box::new(|| Ok(())),
                                tempdir_fn: Box::new(|| Ok(())),
                            },
                            dest,
                        }
                    },
                    |_, dest, res| match res.unwrap_err() {
                        Error::DestinationDirectoryAlreadyExists(path) => assert_eq!(path, dest),
                        err => panic!(
                            "expected DestinationDirectoryAlreadyExists (actual: {:?})",
                            err
                        ),
                    },
                );
            }

            #[test]
            fn err_if_tempdir_failed() {
                test(
                    move |ctx| {
                        let name = "test";
                        Data {
                            params: Parameters {
                                args: NewCommandArguments {
                                    name: name.into(),
                                    tpl: ctx.tpl.into(),
                                    ..NewCommandArguments::default_for_test()
                                },
                                checkout_repo_fn: Box::new(Ok),
                                cwd_fn: Box::new(|| Ok(())),
                                git_ref: None,
                                render_recursively_fn: Box::new(|| Ok(())),
                                tempdir_fn: Box::new(move || {
                                    Err(io::Error::from(io::ErrorKind::PermissionDenied))
                                }),
                            },
                            dest: ctx.cwd.join(name),
                        }
                    },
                    |_, dest, res| match res.unwrap_err() {
                        Error::IO(_) => assert!(!dest.exists()),
                        err => panic!("expected IO (actual: {:?})", err),
                    },
                );
            }

            #[test]
            fn err_if_checkout_failed() {
                test(
                    move |ctx| {
                        let name = "test";
                        Data {
                            params: Parameters {
                                args: NewCommandArguments {
                                    name: name.into(),
                                    tpl: ctx.tpl.into(),
                                    ..NewCommandArguments::default_for_test()
                                },
                                checkout_repo_fn: Box::new(move |_| {
                                    Err(git2::Error::new(
                                        git2::ErrorCode::Ambiguous,
                                        git2::ErrorClass::Callback,
                                        "error",
                                    ))
                                }),
                                cwd_fn: Box::new(|| Ok(())),
                                git_ref: None,
                                render_recursively_fn: Box::new(|| Ok(())),
                                tempdir_fn: Box::new(|| Ok(())),
                            },
                            dest: ctx.cwd.join(name),
                        }
                    },
                    |ctx, dest, res| match res.unwrap_err() {
                        Error::Git(_) => {
                            assert!(!ctx.tpl_repo_path.exists());
                            assert!(!dest.exists());
                        }
                        err => panic!("expected Git (actual: {:?})", err),
                    },
                );
            }

            #[test]
            fn err_if_tpl_not_found() {
                let expected_tpl = "test";
                test(
                    move |ctx| {
                        let name = "test";
                        Data {
                            params: Parameters {
                                args: NewCommandArguments {
                                    name: name.into(),
                                    tpl: expected_tpl.into(),
                                    ..NewCommandArguments::default_for_test()
                                },
                                checkout_repo_fn: Box::new(Ok),
                                cwd_fn: Box::new(|| Ok(())),
                                git_ref: None,
                                render_recursively_fn: Box::new(|| Ok(())),
                                tempdir_fn: Box::new(|| Ok(())),
                            },
                            dest: ctx.cwd.join(name),
                        }
                    },
                    |_, _, res| match res.unwrap_err() {
                        Error::TemplateNotFound(tpl) => {
                            assert_eq!(tpl, expected_tpl);
                        }
                        err => panic!("expected TemplateNotFound (actual: {:?})", err),
                    },
                );
            }

            #[test]
            fn err_if_render_recursively_failed() {
                test(
                    move |ctx| {
                        let name = "test";
                        Data {
                            params: Parameters {
                                args: NewCommandArguments {
                                    name: name.into(),
                                    tpl: ctx.tpl.into(),
                                    ..NewCommandArguments::default_for_test()
                                },
                                checkout_repo_fn: Box::new(Ok),
                                cwd_fn: Box::new(|| Ok(())),
                                git_ref: None,
                                render_recursively_fn: Box::new(move || {
                                    Err(Error::IO(io::Error::from(io::ErrorKind::PermissionDenied)))
                                }),
                                tempdir_fn: Box::new(|| Ok(())),
                            },
                            dest: ctx.cwd.join(name),
                        }
                    },
                    |ctx, dest, res| match res.unwrap_err() {
                        Error::IO(_) => {
                            assert!(!ctx.tpl_repo_path.exists());
                            assert!(!dest.exists());
                        }
                        err => panic!("expected IO (actual: {:?})", err),
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
                                tpl: ctx.tpl.into(),
                                ..NewCommandArguments::default_for_test()
                            },
                            checkout_repo_fn: Box::new(Ok),
                            cwd_fn: Box::new(|| Ok(())),
                            git_ref: None,
                            render_recursively_fn: Box::new(|| Ok(())),
                            tempdir_fn: Box::new(|| Ok(())),
                        },
                        dest: ctx.cwd.join(name),
                    }
                });
            }

            #[test]
            fn ok_when_custom_args() {
                ok(move |ctx| {
                    let dest = tempdir().unwrap().into_path().join("test");
                    let branch = "develop";
                    Data {
                        params: Parameters {
                            args: NewCommandArguments {
                                dest: Some(dest.clone()),
                                git: format!("{}2", DEFAULT_TPL_GIT_REPO_URL),
                                git_branch: Some(branch.into()),
                                git_tag: None,
                                tpl: ctx.tpl.into(),
                                ..NewCommandArguments::default_for_test()
                            },
                            checkout_repo_fn: Box::new(Ok),
                            cwd_fn: Box::new(|| Ok(())),
                            git_ref: Some(Reference::Branch(branch.into())),
                            render_recursively_fn: Box::new(|| Ok(())),
                            tempdir_fn: Box::new(|| Ok(())),
                        },
                        dest,
                    }
                });
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
                                tpl: ctx.tpl.into(),
                                ..NewCommandArguments::default_for_test()
                            },
                            checkout_repo_fn: Box::new(Ok),
                            cwd_fn: Box::new(|| Ok(())),
                            git_ref: Some(Reference::Tag(tag.into())),
                            render_recursively_fn: Box::new(|| Ok(())),
                            tempdir_fn: Box::new(|| Ok(())),
                        },
                        dest: ctx.cwd.join(name),
                    }
                });
            }

            #[inline]
            fn ok<D: Fn(&Context) -> Data>(data_from_fn: D) {
                test(data_from_fn, |ctx, dest, res| {
                    res.unwrap();
                    assert!(dest.exists());
                    assert!(!ctx.tpl_repo_path.exists());
                });
            }

            #[inline]
            fn test<D: Fn(&Context) -> Data, A: Fn(&Context, &Path, Result)>(
                data_from_fn: D,
                assert_fn: A,
            ) {
                let cwd = tempdir().unwrap().into_path();
                let tpl_repo_path = tempdir().unwrap().into_path();
                let tpl = "mytemplate";
                let expected_tpl_dirpath = tpl_repo_path.join(tpl);
                create_dir_all(&expected_tpl_dirpath).unwrap();
                let ctx = Context {
                    cwd: &cwd,
                    tpl,
                    tpl_repo_path: &tpl_repo_path,
                };
                let data = data_from_fn(&ctx);
                let fs = StubFileSystem::new()
                    .with_stub_of_create_dir(|_, path| create_dir_all(path).map_err(Error::IO))
                    .with_stub_of_create_temp_dir({
                        let tpl_repo_path = tpl_repo_path.clone();
                        move |_| {
                            (data.params.tempdir_fn)()
                                .map(|_| tpl_repo_path.clone())
                                .map_err(Error::IO)
                        }
                    })
                    .with_stub_of_cwd({
                        let cwd = cwd.clone();
                        move |_| {
                            (data.params.cwd_fn)()
                                .map(|_| cwd.clone())
                                .map_err(Error::IO)
                        }
                    })
                    .with_stub_of_delete_dir(|_, path| remove_dir_all(path).map_err(Error::IO));
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
                let renderer = StubRenderer::new().with_stub_of_render_recursively({
                    let expected_dest = data.dest.clone();
                    let mut expected_vars = data.params.args.vars.clone();
                    expected_vars.push((NAME_VAR_NAME.into(), data.params.args.name.clone()));
                    move |_, tpl_dirpath, dest, vars| {
                        assert_eq!(tpl_dirpath, expected_tpl_dirpath);
                        assert_eq!(dest, expected_dest);
                        assert_eq!(vars, expected_vars);
                        (data.params.render_recursively_fn)()
                    }
                });
                let cmd = NewCommand {
                    args: data.params.args,
                    fs: Box::new(fs),
                    renderer: Box::new(renderer),
                    git: Box::new(git),
                };
                assert_fn(&ctx, &data.dest, cmd.run());
            }
        }
    }
}
