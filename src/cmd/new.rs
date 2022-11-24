use std::{
    fmt::{self, Debug, Formatter},
    path::Path,
};

use log::{debug, warn};

use crate::{
    cli::NewCommandArguments,
    err::Error,
    fs::{DefaultFileSystem, FileSystem},
    git::{DefaultGit, Git, Reference},
};

use super::{Command, CommandKind, Result};

pub struct NewCommand {
    args: NewCommandArguments,
    fs: Box<dyn FileSystem>,
    git: Box<dyn Git>,
}

impl NewCommand {
    pub fn new(args: NewCommandArguments) -> Self {
        Self {
            args,
            fs: Box::new(DefaultFileSystem),
            git: Box::new(DefaultGit),
        }
    }

    #[inline]
    fn delete_dir(path: &Path, fs: &dyn FileSystem) {
        if let Err(err) = fs.delete_dir(path) {
            warn!("Unable to delete {}: {}", path.display(), err);
        }
    }

    fn render_files_recursively(tpl_dirpath: &Path, dest: &Path, fs: &dyn FileSystem) -> Result {
        debug!("Reading {} directory", tpl_dirpath.display());
        for entry in fs.read_dir(tpl_dirpath)? {
            let entry = entry.map_err(Error::IO)?;
            let path = entry.path();
            let filename = path.file_name().unwrap();
            let dest = dest.join(filename);
            if path.is_dir() {
                fs.create_dir(&dest)?;
                Self::render_files_recursively(&path, &dest, fs)?;
            } else if path.is_file() {
                debug!("Copying {} into {}", path.display(), dest.display());
                fs.copy(&path, &dest)?;
            }
        }
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
                    Self::render_files_recursively(&tpl_dirpath, &dest, self.fs.as_ref())
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
        fs::{copy, create_dir_all, read_to_string, remove_dir_all, File},
        io::{self, Write},
        path::PathBuf,
    };

    use git2::Repository;
    use tempfile::tempdir;

    use crate::{cli::DEFAULT_TPL_GIT_REPO_URL, fs::StubFileSystem, git::StubGit};

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
            type ReadDirFn = dyn Fn() -> io::Result<()>;
            type TempdirFn = dyn Fn(PathBuf) -> io::Result<PathBuf>;

            struct Context<'a> {
                cwd: &'a Path,
                static_file_content: &'a str,
                static_rel_filepath: &'a Path,
                templated_file_content: &'a str,
                templated_rel_filepath: &'a Path,
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
                read_dir_fn: Box<ReadDirFn>,
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
                                cwd_fn: Box::new(Ok),
                                git_ref: None,
                                read_dir_fn: Box::new(|| Ok(())),
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
                                    tpl: ctx.tpl.into(),
                                    ..NewCommandArguments::default_for_test()
                                },
                                checkout_repo_fn: Box::new(Ok),
                                cwd_fn: Box::new(Ok),
                                git_ref: None,
                                read_dir_fn: Box::new(|| Ok(())),
                                tempdir_fn: Box::new(move |_| Err(io::Error::from(err_kind))),
                            },
                            dest: ctx.cwd.join(name),
                        }
                    },
                    |_, dest, res| match res.unwrap_err() {
                        Error::IO(_) => assert!(!dest.is_dir()),
                        err => {
                            let expected_err = Error::IO(io::Error::from(err_kind));
                            panic!("expected {:?} (actual: {:?})", expected_err, err);
                        }
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
                                    tpl: ctx.tpl.into(),
                                    ..NewCommandArguments::default_for_test()
                                },
                                checkout_repo_fn: Box::new(move |_| {
                                    Err(git2::Error::new(err_code, err_class, err_msg))
                                }),
                                cwd_fn: Box::new(Ok),
                                git_ref: None,
                                read_dir_fn: Box::new(|| Ok(())),
                                tempdir_fn: Box::new(Ok),
                            },
                            dest: ctx.cwd.join(name),
                        }
                    },
                    |ctx, dest, res| match res.unwrap_err() {
                        Error::Git(_) => {
                            assert!(!ctx.tpl_repo_path.is_dir());
                            assert!(!dest.is_dir());
                        }
                        err => {
                            let expected_err = git2::Error::new(err_code, err_class, err_msg);
                            panic!("expected {:?} (actual: {:?})", expected_err, err);
                        }
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
                                cwd_fn: Box::new(Ok),
                                git_ref: None,
                                read_dir_fn: Box::new(|| Ok(())),
                                tempdir_fn: Box::new(Ok),
                            },
                            dest: ctx.cwd.join(name),
                        }
                    },
                    |_, _, res| match res.unwrap_err() {
                        Error::TemplateNotFound(tpl) => {
                            assert_eq!(tpl, expected_tpl);
                        }
                        err => {
                            let expected_err = Error::TemplateNotFound(expected_tpl.into());
                            panic!("expected {:?} (actual: {:?})", expected_err, err);
                        }
                    },
                );
            }

            #[test]
            fn err_if_list_dir_failed() {
                let err_kind = io::ErrorKind::PermissionDenied;
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
                                cwd_fn: Box::new(Ok),
                                git_ref: None,
                                read_dir_fn: Box::new(move || Err(io::Error::from(err_kind))),
                                tempdir_fn: Box::new(Ok),
                            },
                            dest: ctx.cwd.join(name),
                        }
                    },
                    |ctx, dest, res| match res.unwrap_err() {
                        Error::IO(_) => {
                            assert!(!ctx.tpl_repo_path.is_dir());
                            assert!(!dest.is_dir());
                        }
                        err => {
                            let expected_err = io::Error::from(err_kind);
                            panic!("expected {:?} (actual: {:?})", expected_err, err);
                        }
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
                            cwd_fn: Box::new(Ok),
                            git_ref: None,
                            read_dir_fn: Box::new(|| Ok(())),
                            tempdir_fn: Box::new(Ok),
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
                            cwd_fn: Box::new(Ok),
                            git_ref: Some(Reference::Branch(branch.into())),
                            read_dir_fn: Box::new(|| Ok(())),
                            tempdir_fn: Box::new(Ok),
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
                            cwd_fn: Box::new(Ok),
                            git_ref: Some(Reference::Tag(tag.into())),
                            read_dir_fn: Box::new(|| Ok(())),
                            tempdir_fn: Box::new(Ok),
                        },
                        dest: ctx.cwd.join(name),
                    }
                });
            }

            #[inline]
            fn ok<D: Fn(&Context) -> Data>(data_from_fn: D) {
                test(data_from_fn, |ctx, dest, res| {
                    res.unwrap();
                    assert!(!ctx.tpl_repo_path.is_dir());
                    let static_filepath = dest.join(ctx.static_rel_filepath);
                    let static_file_content = read_to_string(&static_filepath).unwrap();
                    assert_eq!(static_file_content, ctx.static_file_content);
                    let templated_filepath = dest.join(ctx.templated_rel_filepath);
                    let templated_file_content = read_to_string(&templated_filepath).unwrap();
                    assert_eq!(templated_file_content, ctx.templated_file_content);
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
                let tpl_dirpath = tpl_repo_path.join(tpl);
                let project_src_rel_dirpath = Path::new("{{name}}/src");
                let project_src_dirpath = tpl_dirpath.join(project_src_rel_dirpath);
                create_dir_all(&project_src_dirpath).unwrap();
                let static_rel_filepath = project_src_rel_dirpath.join("static");
                let static_filepath = tpl_dirpath.join(&static_rel_filepath);
                let mut static_file = File::create(&static_filepath).unwrap();
                let static_file_content = "{{name}}";
                write!(static_file, "{}", static_file_content).unwrap();
                drop(static_file);
                let templated_rel_filepath = project_src_rel_dirpath.join("{{name}}.liquid");
                let templated_filepath = tpl_dirpath.join(&templated_rel_filepath);
                let mut templated_file = File::create(&templated_filepath).unwrap();
                write!(templated_file, "{}", static_file_content).unwrap();
                drop(templated_file);
                let ctx = Context {
                    cwd: &cwd,
                    static_file_content,
                    static_rel_filepath: &static_rel_filepath,
                    templated_file_content: static_file_content,
                    templated_rel_filepath: &templated_rel_filepath,
                    tpl,
                    tpl_repo_path: &tpl_repo_path,
                };
                let data = data_from_fn(&ctx);
                let fs = StubFileSystem::new()
                    .with_stub_of_copy(|_, src, dest| {
                        copy(src, dest).map(|_| ()).map_err(Error::IO)
                    })
                    .with_stub_of_create_dir(|_, path| create_dir_all(path).map_err(Error::IO))
                    .with_stub_of_create_temp_dir({
                        let tpl_repo_path = tpl_repo_path.clone();
                        move |_| (data.params.tempdir_fn)(tpl_repo_path.clone()).map_err(Error::IO)
                    })
                    .with_stub_of_cwd({
                        let cwd = cwd.clone();
                        move |_| (data.params.cwd_fn)(cwd.clone()).map_err(Error::IO)
                    })
                    .with_stub_of_delete_dir(|_, path| remove_dir_all(path).map_err(Error::IO))
                    .with_stub_of_read_dir(move |_, path| {
                        (data.params.read_dir_fn)()
                            .and_then(|_| path.read_dir())
                            .map_err(Error::IO)
                    });
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
                    fs: Box::new(fs),
                    git: Box::new(git),
                };
                assert_fn(&ctx, &data.dest, cmd.run());
            }
        }
    }
}
