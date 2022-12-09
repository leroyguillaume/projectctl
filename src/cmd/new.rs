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
    renderer::{LiquidRenderer, Renderer, Vars},
};

use super::Result;

const DESCRIPTION_VAR_KEY: &str = "description";
const GIT_USER_EMAIL_VAR_KEY: &str = "git_user_email";
const GIT_USER_NAME_VAR_KEY: &str = "git_user_name";
const NAME_VAR_NAME: &str = "name";

const GIT_USER_EMAIL_CONFIG_KEY: &str = "user.email";
const GIT_USER_NAME_CONFIG_KEY: &str = "user.name";

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
            git: Box::new(DefaultGit::new()),
            renderer: Box::new(LiquidRenderer::new()),
        }
    }

    pub fn run(self) -> Result {
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
            .and_then(|_| {
                let tpl_dirpath = tpl_repo_path.join(&self.args.tpl);
                if tpl_dirpath.is_dir() {
                    let vars = Self::create_vars(
                        self.args.name,
                        self.args.desc,
                        self.args.vars,
                        self.git.as_ref(),
                    );
                    self.renderer.render_recursively(&tpl_dirpath, &dest, vars)
                } else {
                    Err(Error::TemplateNotFound(self.args.tpl))
                }
            });
        Self::delete_dir(&tpl_repo_path, self.fs.as_ref());
        if res.is_ok() {
            if let Err(err) = self.git.init(&dest) {
                warn!("{}", err);
            }
        } else {
            Self::delete_dir(&dest, self.fs.as_ref());
        }
        res
    }

    #[inline]
    fn create_vars(
        name: String,
        desc: Option<String>,
        cli_vars: Vec<(String, String)>,
        git: &dyn Git,
    ) -> Vars {
        let mut vars = Vars::new();
        Self::inject_var(NAME_VAR_NAME.into(), name, &mut vars);
        if let Some(desc) = desc {
            Self::inject_var(DESCRIPTION_VAR_KEY.into(), desc, &mut vars);
        }
        Self::inject_git_var(
            GIT_USER_NAME_VAR_KEY,
            GIT_USER_NAME_CONFIG_KEY,
            &mut vars,
            git,
        );
        Self::inject_git_var(
            GIT_USER_EMAIL_VAR_KEY,
            GIT_USER_EMAIL_CONFIG_KEY,
            &mut vars,
            git,
        );
        for (key, val) in cli_vars {
            Self::inject_var(key, val, &mut vars);
        }
        vars
    }

    #[inline]
    fn delete_dir(path: &Path, fs: &dyn FileSystem) {
        if let Err(err) = fs.delete_dir(path) {
            warn!("Unable to delete {}: {}", path.display(), err);
        }
    }

    #[inline]
    fn inject_git_var(key: &str, git_cfg_key: &str, vars: &mut Vars, git: &dyn Git) {
        match git.default_config_value(git_cfg_key) {
            Ok(val) => Self::inject_var(key.into(), val, vars),
            Err(err) => warn!("{}", err),
        }
    }

    #[inline]
    fn inject_var(key: String, val: String, vars: &mut Vars) {
        if let Some(prev_value) = vars.insert(key.clone(), val.clone()) {
            warn!(
                "Variable `{}` is overriden (`{}` over `{}`)",
                key, val, prev_value
            );
        }
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
        collections::HashMap,
        fs::{create_dir_all, remove_dir_all},
        io::{self},
        path::PathBuf,
    };

    use git2::Repository;
    use tempfile::tempdir;

    use crate::{err::LiquidErrorSource, fs::StubFileSystem, git::StubGit, renderer::StubRenderer};

    use super::*;

    mod new_command {
        use super::*;

        mod new {
            use super::*;

            #[test]
            fn cmd() {
                let args = NewCommandArguments::new("test".into(), "my-project".into());
                let cmd = NewCommand::new(args.clone());
                assert_eq!(cmd.args, args);
            }
        }

        mod run {
            use super::*;

            struct Context {
                cwd: PathBuf,
                git_email: &'static str,
                git_username: &'static str,
                name: &'static str,
                tpl: &'static str,
                tpl_repo_path: PathBuf,
            }

            struct Expected {
                dest: PathBuf,
                git_ref: Option<Reference>,
                vars: HashMap<String, String>,
            }

            struct Parameters {
                args: NewCommandArguments,
                fail_cwd: bool,
                fail_git_checking_out: bool,
                fail_git_default_value_retrieving: bool,
                fail_dir_deletion: bool,
                fail_git_init: bool,
                fail_redering: bool,
            }

            #[test]
            fn err_when_dest_already_exists() {
                let dest = tempdir().unwrap().into_path();
                test(
                    |ctx| Parameters {
                        args: NewCommandArguments {
                            dest: Some(dest.clone()),
                            ..NewCommandArguments::new(ctx.tpl.into(), ctx.name.into())
                        },
                        fail_cwd: false,
                        fail_dir_deletion: false,
                        fail_git_default_value_retrieving: false,
                        fail_git_checking_out: false,
                        fail_git_init: false,
                        fail_redering: false,
                    },
                    |_| Expected {
                        dest: dest.clone(),
                        git_ref: None,
                        vars: HashMap::new(),
                    },
                    |_, res| match res.unwrap_err() {
                        Error::DestinationDirectoryAlreadyExists(path) => assert_eq!(path, dest),
                        err => panic!(
                            "expected DestinationDirectoryAlreadyExists (actual: {:?})",
                            err
                        ),
                    },
                );
            }

            #[test]
            fn err_when_checkout_failed() {
                let dest_fn = |ctx: &Context| -> PathBuf { ctx.cwd.join(ctx.name) };
                test(
                    |ctx| Parameters {
                        args: NewCommandArguments::new(ctx.tpl.into(), ctx.name.into()),
                        fail_cwd: false,
                        fail_dir_deletion: false,
                        fail_git_default_value_retrieving: false,
                        fail_git_checking_out: true,
                        fail_git_init: false,
                        fail_redering: false,
                    },
                    |ctx| Expected {
                        dest: dest_fn(ctx),
                        git_ref: None,
                        vars: HashMap::new(),
                    },
                    |ctx, res| {
                        match res.unwrap_err() {
                            Error::Git(_) => (),
                            err => panic!("expected Git (actual: {:?})", err),
                        }
                        let dest = dest_fn(ctx);
                        assert!(!dest.exists());
                        assert!(!ctx.tpl_repo_path.exists());
                    },
                );
            }

            #[test]
            fn err_when_tpl_does_not_exist() {
                let dest_fn = |ctx: &Context| -> PathBuf { ctx.cwd.join(ctx.name) };
                let tpl_fn = |ctx: &Context| -> String { format!("{}2", ctx.tpl) };
                test(
                    |ctx| Parameters {
                        args: NewCommandArguments::new(tpl_fn(ctx), ctx.name.into()),
                        fail_cwd: false,
                        fail_dir_deletion: false,
                        fail_git_default_value_retrieving: false,
                        fail_git_checking_out: false,
                        fail_git_init: false,
                        fail_redering: false,
                    },
                    |ctx| Expected {
                        dest: dest_fn(ctx),
                        git_ref: None,
                        vars: HashMap::new(),
                    },
                    |ctx, res| {
                        let expected_tpl = tpl_fn(ctx);
                        match res.unwrap_err() {
                            Error::TemplateNotFound(tpl) => assert_eq!(tpl, expected_tpl),
                            err => panic!("expected Git (actual: {:?})", err),
                        }
                        let dest = dest_fn(ctx);
                        assert!(!dest.exists());
                        assert!(!ctx.tpl_repo_path.exists());
                    },
                );
            }

            #[test]
            fn err_when_tpl_does_not_exist_and_dir_deletion_failed() {
                let dest_fn = |ctx: &Context| -> PathBuf { ctx.cwd.join(ctx.name) };
                let tpl_fn = |ctx: &Context| -> String { format!("{}2", ctx.tpl) };
                test(
                    |ctx| Parameters {
                        args: NewCommandArguments::new(tpl_fn(ctx), ctx.name.into()),
                        fail_cwd: false,
                        fail_dir_deletion: true,
                        fail_git_default_value_retrieving: false,
                        fail_git_checking_out: false,
                        fail_git_init: false,
                        fail_redering: false,
                    },
                    |ctx| Expected {
                        dest: dest_fn(ctx),
                        git_ref: None,
                        vars: HashMap::new(),
                    },
                    |ctx, res| {
                        let expected_tpl = tpl_fn(ctx);
                        match res.unwrap_err() {
                            Error::TemplateNotFound(tpl) => assert_eq!(tpl, expected_tpl),
                            err => panic!("expected Git (actual: {:?})", err),
                        }
                        let dest = dest_fn(ctx);
                        assert!(dest.exists());
                        assert!(ctx.tpl_repo_path.exists());
                    },
                );
            }

            #[test]
            fn err_when_rendering_failed() {
                let dest_fn = |ctx: &Context| -> PathBuf { ctx.cwd.join(ctx.name) };
                test(
                    |ctx| Parameters {
                        args: NewCommandArguments::new(ctx.tpl.into(), ctx.name.into()),
                        fail_cwd: false,
                        fail_dir_deletion: false,
                        fail_git_default_value_retrieving: false,
                        fail_git_checking_out: false,
                        fail_git_init: false,
                        fail_redering: true,
                    },
                    |ctx| Expected {
                        dest: dest_fn(ctx),
                        git_ref: None,
                        vars: HashMap::from_iter([
                            (GIT_USER_EMAIL_VAR_KEY.into(), ctx.git_email.into()),
                            (GIT_USER_NAME_VAR_KEY.into(), ctx.git_username.into()),
                            (NAME_VAR_NAME.into(), ctx.name.into()),
                        ]),
                    },
                    |ctx, res| {
                        match res.unwrap_err() {
                            Error::Liquid { .. } => (),
                            err => panic!("expected Liquid (actual: {:?})", err),
                        }
                        let dest = dest_fn(ctx);
                        assert!(!dest.exists());
                        assert!(!ctx.tpl_repo_path.exists());
                    },
                );
            }

            #[test]
            fn ok_when_git_default_value_retrieving_failed() {
                let dest_fn = |ctx: &Context| -> PathBuf { ctx.cwd.join(ctx.name) };
                test(
                    |ctx| Parameters {
                        args: NewCommandArguments::new(ctx.tpl.into(), ctx.name.into()),
                        fail_cwd: false,
                        fail_dir_deletion: false,
                        fail_git_default_value_retrieving: true,
                        fail_git_checking_out: false,
                        fail_git_init: false,
                        fail_redering: false,
                    },
                    |ctx| Expected {
                        dest: dest_fn(ctx),
                        git_ref: None,
                        vars: HashMap::from_iter([(NAME_VAR_NAME.into(), ctx.name.into())]),
                    },
                    |ctx, res| {
                        res.unwrap();
                        let dest = dest_fn(ctx);
                        assert!(dest.exists());
                        assert!(!ctx.tpl_repo_path.exists());
                    },
                );
            }

            #[test]
            fn ok_when_git_init_failed() {
                let dest_fn = |ctx: &Context| -> PathBuf { ctx.cwd.join(ctx.name) };
                test(
                    |ctx| Parameters {
                        args: NewCommandArguments::new(ctx.tpl.into(), ctx.name.into()),
                        fail_cwd: false,
                        fail_dir_deletion: false,
                        fail_git_default_value_retrieving: false,
                        fail_git_checking_out: false,
                        fail_git_init: true,
                        fail_redering: false,
                    },
                    |ctx| Expected {
                        dest: dest_fn(ctx),
                        git_ref: None,
                        vars: HashMap::from_iter([
                            (GIT_USER_EMAIL_VAR_KEY.into(), ctx.git_email.into()),
                            (GIT_USER_NAME_VAR_KEY.into(), ctx.git_username.into()),
                            (NAME_VAR_NAME.into(), ctx.name.into()),
                        ]),
                    },
                    |ctx, res| {
                        res.unwrap();
                        let dest = dest_fn(ctx);
                        assert!(dest.exists());
                        assert!(!ctx.tpl_repo_path.exists());
                    },
                );
            }

            #[test]
            fn ok_when_custom_args() {
                let desc = "My wonderful project.";
                let dest = tempdir().unwrap().into_path().join("test");
                let tpl_repo_url = "https://my-templates.com";
                let tpl_repo_branch = "develop";
                let var_key = "VAR";
                let var_val = "VAL";
                test(
                    |ctx| Parameters {
                        args: NewCommandArguments {
                            desc: Some(desc.into()),
                            dest: Some(dest.clone()),
                            git: tpl_repo_url.into(),
                            git_branch: Some(tpl_repo_branch.into()),
                            git_tag: None,
                            name: ctx.name.into(),
                            tpl: ctx.tpl.into(),
                            vars: vec![
                                (var_key.into(), format!("{}2", var_val)),
                                (var_key.into(), var_val.into()),
                            ],
                        },
                        fail_cwd: false,
                        fail_dir_deletion: false,
                        fail_git_default_value_retrieving: false,
                        fail_git_checking_out: false,
                        fail_git_init: true,
                        fail_redering: false,
                    },
                    |ctx| Expected {
                        dest: dest.clone(),
                        git_ref: Some(Reference::Branch(tpl_repo_branch.into())),
                        vars: HashMap::from_iter([
                            (GIT_USER_EMAIL_VAR_KEY.into(), ctx.git_email.into()),
                            (GIT_USER_NAME_VAR_KEY.into(), ctx.git_username.into()),
                            (NAME_VAR_NAME.into(), ctx.name.into()),
                            (DESCRIPTION_VAR_KEY.into(), desc.into()),
                            (var_key.into(), var_val.into()),
                        ]),
                    },
                    |ctx, res| {
                        res.unwrap();
                        assert!(dest.exists());
                        assert!(!ctx.tpl_repo_path.exists());
                    },
                );
            }

            #[test]
            fn ok_when_tpl_repo_tag_is_some() {
                let dest_fn = |ctx: &Context| -> PathBuf { ctx.cwd.join(ctx.name) };
                let tpl_repo_tag = "0.1.0";
                test(
                    |ctx| Parameters {
                        args: NewCommandArguments {
                            git_tag: Some(tpl_repo_tag.into()),
                            ..NewCommandArguments::new(ctx.tpl.into(), ctx.name.into())
                        },
                        fail_cwd: false,
                        fail_dir_deletion: false,
                        fail_git_default_value_retrieving: false,
                        fail_git_checking_out: false,
                        fail_git_init: true,
                        fail_redering: false,
                    },
                    |ctx| Expected {
                        dest: dest_fn(ctx),
                        git_ref: Some(Reference::Tag(tpl_repo_tag.into())),
                        vars: HashMap::from_iter([
                            (GIT_USER_EMAIL_VAR_KEY.into(), ctx.git_email.into()),
                            (GIT_USER_NAME_VAR_KEY.into(), ctx.git_username.into()),
                            (NAME_VAR_NAME.into(), ctx.name.into()),
                        ]),
                    },
                    |ctx, res| {
                        res.unwrap();
                        let dest = dest_fn(ctx);
                        assert!(dest.exists());
                        assert!(!ctx.tpl_repo_path.exists());
                    },
                );
            }

            fn test<
                P: Fn(&Context) -> Parameters,
                E: Fn(&Context) -> Expected,
                A: Fn(&Context, Result),
            >(
                create_params_fn: P,
                create_expected_fn: E,
                assert_fn: A,
            ) {
                let ctx = Context {
                    cwd: tempdir().unwrap().into_path(),
                    name: "my-project",
                    git_email: "test@local",
                    git_username: "test",
                    tpl: "test",
                    tpl_repo_path: tempdir().unwrap().into_path(),
                };
                let params = create_params_fn(&ctx);
                let expected = create_expected_fn(&ctx);
                create_dir_all(ctx.tpl_repo_path.join(ctx.tpl)).unwrap();
                let fs = StubFileSystem::new()
                    .with_stub_of_create_dir(|_, path| create_dir_all(path).map_err(Error::IO))
                    .with_stub_of_create_temp_dir({
                        let tpl_repo_path = ctx.tpl_repo_path.clone();
                        move |_| Ok(tpl_repo_path.clone())
                    })
                    .with_stub_of_cwd({
                        let cwd = ctx.cwd.clone();
                        move |_| {
                            if params.fail_cwd {
                                Err(Error::IO(io::Error::from(io::ErrorKind::PermissionDenied)))
                            } else {
                                Ok(cwd.clone())
                            }
                        }
                    })
                    .with_stub_of_delete_dir(move |_, path| {
                        if params.fail_dir_deletion {
                            Err(Error::IO(io::Error::from(io::ErrorKind::PermissionDenied)))
                        } else {
                            remove_dir_all(path).map_err(Error::IO)
                        }
                    });
                let git = StubGit::new()
                    .with_stub_of_checkout_repository({
                        let expected_url = params.args.git.clone();
                        let tpl_repo_path = ctx.tpl_repo_path.clone();
                        move |_, url, reference, dest| {
                            assert_eq!(url, expected_url);
                            assert_eq!(reference, expected.git_ref);
                            assert_eq!(dest, tpl_repo_path);
                            if params.fail_git_checking_out {
                                Err(Error::Git(git2::Error::from_str("error")))
                            } else {
                                Repository::init(&tpl_repo_path).map_err(Error::Git)
                            }
                        }
                    })
                    .with_stub_of_default_config_value(move |i, key| {
                        let val = if i == 0 {
                            assert_eq!(key, GIT_USER_NAME_CONFIG_KEY);
                            ctx.git_username
                        } else if i == 1 {
                            assert_eq!(key, GIT_USER_EMAIL_CONFIG_KEY);
                            ctx.git_email
                        } else {
                            panic!("unexpected key `{}`", key);
                        };
                        if params.fail_git_default_value_retrieving {
                            Err(Error::Git(git2::Error::from_str("error")))
                        } else {
                            Ok(val.into())
                        }
                    })
                    .with_stub_of_init({
                        let expected_dest = expected.dest.clone();
                        move |_, path| {
                            assert_eq!(path, expected_dest);
                            if params.fail_git_init {
                                Err(Error::Git(git2::Error::from_str("error")))
                            } else {
                                Repository::init(path).map_err(Error::Git)
                            }
                        }
                    });
                let renderer = StubRenderer::new().with_stub_of_render_recursively({
                    let tpl_repo_path = ctx.tpl_repo_path.clone();
                    let expected_dest = expected.dest.clone();
                    let tpl = params.args.tpl.clone();
                    move |_, tpl_dirpath, dest, vars| {
                        assert_eq!(tpl_dirpath, tpl_repo_path.join(&tpl));
                        assert_eq!(dest, expected_dest);
                        assert_eq!(vars, expected.vars);
                        if params.fail_redering {
                            Err(Error::Liquid {
                                cause: liquid::Error::with_msg("error"),
                                src: LiquidErrorSource::Filename("test".into()),
                            })
                        } else {
                            Ok(())
                        }
                    }
                });
                let cmd = NewCommand {
                    args: params.args,
                    fs: Box::new(fs),
                    renderer: Box::new(renderer),
                    git: Box::new(git),
                };
                let res = cmd.run();
                assert_fn(&ctx, res);
            }
        }
    }
}
