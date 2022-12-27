use std::{
    collections::HashMap,
    fmt::{self, Debug, Formatter},
    path::Path,
};

use log::{debug, info, warn};

use crate::{
    cli::NewCommandArguments,
    err::{Error, Result},
    fs::{DefaultFileSystem, FileSystem},
    git::{DefaultGit, Git, Reference},
    paths::{DefaultPaths, Paths, LOCAL_CONFIG_FILENAME},
    renderer::{LiquidRenderer, Renderer},
};

const DESCRIPTION_VAR_KEY: &str = "description";
const GIT_USER_EMAIL_VAR_KEY: &str = "git_user_email";
const GIT_USER_NAME_VAR_KEY: &str = "git_user_name";
const NAME_VAR_NAME: &str = "name";

const GIT_USER_EMAIL_CONFIG_KEY: &str = "user.email";
const GIT_USER_NAME_CONFIG_KEY: &str = "user.name";

const GITIGNORE_FILENAME: &str = ".gitignore";
const FILENAMES_TO_IGNORE: [&str; 1] = [LOCAL_CONFIG_FILENAME];

pub struct NewCommand {
    args: NewCommandArguments,
    fs: Box<dyn FileSystem>,
    git: Box<dyn Git>,
    paths: Box<dyn Paths>,
    renderer: Box<dyn Renderer>,
    vars_loader: Box<dyn VariablesLoader>,
}

impl NewCommand {
    pub fn new(args: NewCommandArguments) -> Self {
        Self {
            args,
            fs: Box::new(DefaultFileSystem),
            git: Box::new(DefaultGit::new()),
            paths: Box::new(DefaultPaths::new()),
            renderer: Box::new(LiquidRenderer::new()),
            vars_loader: Box::new(DefaultVariablesLoader::new()),
        }
    }

    pub fn run(self) -> Result<()> {
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
            .tpl_repo_branch
            .map(Reference::Branch)
            .or_else(|| self.args.tpl_repo_tag.map(Reference::Tag));
        let res = self
            .git
            .checkout_repository(&self.args.tpl_repo_url, git_ref, &tpl_repo_path)
            .and_then(|_| {
                let tpl_dirpath = tpl_repo_path.join(&self.args.tpl);
                if tpl_dirpath.is_dir() {
                    let vars =
                        self.vars_loader
                            .load(self.args.name, self.args.desc, self.args.vars);
                    self.renderer.render_recursively(&tpl_dirpath, &dest, vars)
                } else {
                    Err(Error::TemplateNotFound(self.args.tpl))
                }
            });
        Self::delete_dir(&tpl_repo_path, self.fs.as_ref());
        if res.is_ok() {
            info!("Updating allowed directories list");
            let res = self
                .paths
                .allowed_dirs(self.args.allowed_dirs_filepath, None)
                .and_then(|path| {
                    self.fs
                        .ensure_lines(&[&dest.to_string_lossy()], &path, true)
                });
            if let Err(err) = res {
                warn!("{}", err);
            }
            info!("Initializing git repository");
            if let Err(err) = self.git.init(&dest) {
                warn!("{}", err);
            }
            if !self.args.skip_gitignore_update {
                info!("Updating gitignore");
                if let Err(err) = self.fs.ensure_lines(
                    &FILENAMES_TO_IGNORE,
                    &dest.join(GITIGNORE_FILENAME),
                    false,
                ) {
                    warn!("{}", err);
                }
            }
        } else {
            Self::delete_dir(&dest, self.fs.as_ref());
        }
        res
    }

    #[inline]
    fn delete_dir(path: &Path, fs: &dyn FileSystem) {
        if let Err(err) = fs.delete_dir(path) {
            warn!("Unable to delete {}: {}", path.display(), err);
        }
    }
}

impl Debug for NewCommand {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        f.debug_struct("NewCommand")
            .field("args", &self.args)
            .finish()
    }
}

#[cfg_attr(test, stub_trait::stub)]
trait VariablesLoader {
    fn load(
        &self,
        name: String,
        desc: Option<String>,
        overrides: Vec<(String, String)>,
    ) -> HashMap<String, String>;
}

struct DefaultVariablesLoader {
    git: Box<dyn Git>,
}

impl DefaultVariablesLoader {
    fn new() -> Self {
        Self {
            git: Box::new(DefaultGit::new()),
        }
    }

    fn load_git_var(var_key: &str, git: &dyn Git) -> Option<String> {
        match git.default_config_value(var_key) {
            Ok(val) => Some(val),
            Err(err) => {
                warn!("{}", err);
                None
            }
        }
    }
}

impl VariablesLoader for DefaultVariablesLoader {
    fn load(
        &self,
        name: String,
        desc: Option<String>,
        overrides: Vec<(String, String)>,
    ) -> HashMap<String, String> {
        let mut vars = HashMap::new();
        vars.insert(NAME_VAR_NAME.into(), name);
        if let Some(desc) = desc {
            vars.insert(DESCRIPTION_VAR_KEY.into(), desc);
        }
        if let Some(val) = Self::load_git_var(GIT_USER_NAME_CONFIG_KEY, self.git.as_ref()) {
            vars.insert(GIT_USER_NAME_VAR_KEY.into(), val);
        }
        if let Some(val) = Self::load_git_var(GIT_USER_EMAIL_CONFIG_KEY, self.git.as_ref()) {
            vars.insert(GIT_USER_EMAIL_VAR_KEY.into(), val);
        }
        for (key, val) in overrides {
            if let Some(prev_val) = vars.insert(key.clone(), val.clone()) {
                warn!(
                    "`{}` overriden by `{}` (previous value: `{}`)",
                    key, val, prev_val
                );
            }
        }
        vars
    }
}

#[cfg(test)]
mod test {
    use std::{
        collections::HashMap,
        fs::{create_dir_all, remove_dir_all, write},
        io::{self},
        path::PathBuf,
    };

    use git2::Repository;
    use tempfile::tempdir;

    use crate::{
        err::LiquidErrorSource, fs::StubFileSystem, git::StubGit, paths::StubPaths,
        renderer::StubRenderer,
    };

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
                allowed_dirs_filepath: PathBuf,
                cwd: PathBuf,
                name: &'static str,
                tpl: &'static str,
                tpl_repo_path: PathBuf,
                vars: HashMap<String, String>,
            }

            struct Expected {
                dest: PathBuf,
                git_ref: Option<Reference>,
            }

            struct Parameters {
                args: NewCommandArguments,
                fail_allowed_dirs_path_computing: bool,
                fail_allowed_dirs_updating: bool,
                fail_cwd: bool,
                fail_dir_deletion: bool,
                fail_git_checking_out: bool,
                fail_git_init: bool,
                fail_gitignore_update: bool,
                fail_rendering: bool,
                gitignore_content: Option<String>,
            }

            #[test]
            fn err_when_dest_already_exists() {
                let dest = tempdir().unwrap().into_path();
                test(
                    |ctx| Parameters {
                        fail_allowed_dirs_path_computing: false,
                        args: NewCommandArguments {
                            dest: Some(dest.clone()),
                            ..NewCommandArguments::new(ctx.tpl.into(), ctx.name.into())
                        },
                        fail_allowed_dirs_updating: false,
                        fail_cwd: false,
                        fail_dir_deletion: false,
                        fail_git_checking_out: false,
                        fail_git_init: false,
                        fail_gitignore_update: false,
                        fail_rendering: false,
                        gitignore_content: None,
                    },
                    |_| Expected {
                        dest: dest.clone(),
                        git_ref: None,
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
                        fail_allowed_dirs_path_computing: false,
                        fail_allowed_dirs_updating: false,
                        fail_cwd: false,
                        fail_dir_deletion: false,
                        fail_git_checking_out: true,
                        fail_git_init: false,
                        fail_gitignore_update: false,
                        fail_rendering: false,
                        gitignore_content: None,
                    },
                    |ctx| Expected {
                        dest: dest_fn(ctx),
                        git_ref: None,
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
                        fail_allowed_dirs_path_computing: false,
                        fail_allowed_dirs_updating: false,
                        fail_cwd: false,
                        fail_dir_deletion: false,
                        fail_git_checking_out: false,
                        fail_git_init: false,
                        fail_gitignore_update: false,
                        fail_rendering: false,
                        gitignore_content: None,
                    },
                    |ctx| Expected {
                        dest: dest_fn(ctx),
                        git_ref: None,
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
                        fail_allowed_dirs_path_computing: false,
                        fail_allowed_dirs_updating: false,
                        fail_cwd: false,
                        fail_dir_deletion: true,
                        fail_git_checking_out: false,
                        fail_git_init: false,
                        fail_gitignore_update: false,
                        fail_rendering: false,
                        gitignore_content: None,
                    },
                    |ctx| Expected {
                        dest: dest_fn(ctx),
                        git_ref: None,
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
                        fail_allowed_dirs_path_computing: false,
                        fail_allowed_dirs_updating: false,
                        fail_cwd: false,
                        fail_dir_deletion: false,
                        fail_git_checking_out: false,
                        fail_git_init: false,
                        fail_gitignore_update: false,
                        fail_rendering: true,
                        gitignore_content: None,
                    },
                    |ctx| Expected {
                        dest: dest_fn(ctx),
                        git_ref: None,
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
            fn ok_when_git_init_failed() {
                let dest_fn = |ctx: &Context| -> PathBuf { ctx.cwd.join(ctx.name) };
                test(
                    |ctx| Parameters {
                        args: NewCommandArguments::new(ctx.tpl.into(), ctx.name.into()),
                        fail_allowed_dirs_path_computing: false,
                        fail_allowed_dirs_updating: false,
                        fail_cwd: false,
                        fail_dir_deletion: false,
                        fail_git_checking_out: false,
                        fail_git_init: true,
                        fail_gitignore_update: false,
                        fail_rendering: false,
                        gitignore_content: None,
                    },
                    |ctx| Expected {
                        dest: dest_fn(ctx),
                        git_ref: None,
                    },
                    |ctx, res| {
                        assert(ctx, res, dest_fn(ctx));
                    },
                );
            }

            #[test]
            fn ok_when_gitignore_update_failed() {
                let dest_fn = |ctx: &Context| -> PathBuf { ctx.cwd.join(ctx.name) };
                test(
                    |ctx| Parameters {
                        args: NewCommandArguments::new(ctx.tpl.into(), ctx.name.into()),
                        fail_allowed_dirs_path_computing: false,
                        fail_allowed_dirs_updating: false,
                        fail_cwd: false,
                        fail_dir_deletion: false,
                        fail_git_checking_out: false,
                        fail_git_init: false,
                        fail_gitignore_update: true,
                        fail_rendering: false,
                        gitignore_content: None,
                    },
                    |ctx| Expected {
                        dest: dest_fn(ctx),
                        git_ref: None,
                    },
                    |ctx, res| {
                        assert(ctx, res, dest_fn(ctx));
                    },
                );
            }

            #[test]
            fn ok_when_custom_args() {
                let allowed_dirs_filepath = tempdir().unwrap().into_path().join("allowed-dirs");
                let desc = "My wonderful project.";
                let dest = tempdir().unwrap().into_path().join("test");
                let tpl_repo_url = "https://my-templates.com";
                let tpl_repo_branch = "develop";
                let var_key = "VAR";
                let var_val = "VAL";
                test(
                    |ctx| Parameters {
                        args: NewCommandArguments {
                            allowed_dirs_filepath: Some(allowed_dirs_filepath.clone()),
                            desc: Some(desc.into()),
                            dest: Some(dest.clone()),
                            tpl_repo_url: tpl_repo_url.into(),
                            tpl_repo_branch: Some(tpl_repo_branch.into()),
                            tpl_repo_tag: None,
                            name: ctx.name.into(),
                            skip_gitignore_update: true,
                            tpl: ctx.tpl.into(),
                            vars: vec![
                                (var_key.into(), format!("{}2", var_val)),
                                (var_key.into(), var_val.into()),
                            ],
                        },
                        fail_allowed_dirs_path_computing: false,
                        fail_allowed_dirs_updating: false,
                        fail_cwd: false,
                        fail_dir_deletion: false,
                        fail_git_checking_out: false,
                        fail_git_init: true,
                        fail_gitignore_update: false,
                        fail_rendering: false,
                        gitignore_content: None,
                    },
                    |_| Expected {
                        dest: dest.clone(),
                        git_ref: Some(Reference::Branch(tpl_repo_branch.into())),
                    },
                    |ctx, res| {
                        assert(ctx, res, dest.clone());
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
                            tpl_repo_tag: Some(tpl_repo_tag.into()),
                            ..NewCommandArguments::new(ctx.tpl.into(), ctx.name.into())
                        },
                        fail_allowed_dirs_path_computing: false,
                        fail_allowed_dirs_updating: false,
                        fail_cwd: false,
                        fail_dir_deletion: false,
                        fail_git_checking_out: false,
                        fail_git_init: true,
                        fail_gitignore_update: false,
                        fail_rendering: false,
                        gitignore_content: None,
                    },
                    |ctx| Expected {
                        dest: dest_fn(ctx),
                        git_ref: Some(Reference::Tag(tpl_repo_tag.into())),
                    },
                    |ctx, res| {
                        assert(ctx, res, dest_fn(ctx));
                    },
                );
            }

            #[test]
            fn ok_when_allowed_dirs_path_computing_failed() {
                let dest_fn = |ctx: &Context| -> PathBuf { ctx.cwd.join(ctx.name) };
                test(
                    |ctx| Parameters {
                        args: NewCommandArguments::new(ctx.tpl.into(), ctx.name.into()),
                        fail_allowed_dirs_path_computing: true,
                        fail_allowed_dirs_updating: false,
                        fail_cwd: false,
                        fail_dir_deletion: false,
                        fail_git_checking_out: false,
                        fail_git_init: true,
                        fail_gitignore_update: false,
                        fail_rendering: false,
                        gitignore_content: None,
                    },
                    |ctx| Expected {
                        dest: dest_fn(ctx),
                        git_ref: None,
                    },
                    |ctx, res| {
                        assert(ctx, res, dest_fn(ctx));
                    },
                );
            }

            #[test]
            fn ok_when_allowed_dirs_update_failed() {
                let dest_fn = |ctx: &Context| -> PathBuf { ctx.cwd.join(ctx.name) };
                test(
                    |ctx| Parameters {
                        args: NewCommandArguments::new(ctx.tpl.into(), ctx.name.into()),
                        fail_allowed_dirs_path_computing: false,
                        fail_allowed_dirs_updating: true,
                        fail_cwd: false,
                        fail_dir_deletion: false,
                        fail_git_checking_out: false,
                        fail_git_init: true,
                        fail_gitignore_update: false,
                        fail_rendering: false,
                        gitignore_content: None,
                    },
                    |ctx| Expected {
                        dest: dest_fn(ctx),
                        git_ref: None,
                    },
                    |ctx, res| {
                        assert(ctx, res, dest_fn(ctx));
                    },
                );
            }

            fn assert(ctx: &Context, res: Result<()>, dest: PathBuf) {
                res.unwrap();
                assert!(dest.exists());
                assert!(!ctx.tpl_repo_path.exists());
            }

            fn test<
                P: Fn(&Context) -> Parameters,
                E: Fn(&Context) -> Expected,
                A: Fn(&Context, Result<()>),
            >(
                create_params_fn: P,
                create_expected_fn: E,
                assert_fn: A,
            ) {
                let ctx = Context {
                    allowed_dirs_filepath: tempdir().unwrap().into_path().join("allowed-dirs"),
                    cwd: tempdir().unwrap().into_path(),
                    name: "my-project",
                    tpl: "test",
                    tpl_repo_path: tempdir().unwrap().into_path(),
                    vars: HashMap::from_iter([("key".into(), "val".into())]),
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
                    })
                    .with_stub_of_ensure_lines({
                        let allowed_dirs_filepath = ctx.allowed_dirs_filepath.clone();
                        let dest = expected.dest.clone();
                        move |i, lines, path, lock| {
                            if i == 0 && !params.fail_allowed_dirs_path_computing {
                                assert_eq!(lines, [dest.to_string_lossy().to_string()]);
                                assert_eq!(path, allowed_dirs_filepath);
                                assert!(lock);
                                if params.fail_allowed_dirs_updating {
                                    Err(Error::IO(io::Error::from(io::ErrorKind::PermissionDenied)))
                                } else {
                                    Ok(())
                                }
                            } else if i == 0 && params.fail_allowed_dirs_path_computing
                                || i == 1 && !params.args.skip_gitignore_update
                            {
                                assert_eq!(lines, FILENAMES_TO_IGNORE);
                                assert_eq!(path, dest.join(GITIGNORE_FILENAME));
                                assert!(!lock);
                                if params.fail_gitignore_update {
                                    Err(Error::IO(io::Error::from(io::ErrorKind::PermissionDenied)))
                                } else {
                                    Ok(())
                                }
                            } else {
                                panic!("unexpected call of open");
                            }
                        }
                    });
                let git = StubGit::new()
                    .with_stub_of_checkout_repository({
                        let expected_url = params.args.tpl_repo_url.clone();
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
                let paths = StubPaths::new().with_stub_of_allowed_dirs({
                    let filepath = ctx.allowed_dirs_filepath.clone();
                    let expected_allowed_dirs_filepath = params.args.allowed_dirs_filepath.clone();
                    move |_, allowed_dirs_filepath, cfg_filepath| {
                        assert_eq!(allowed_dirs_filepath, expected_allowed_dirs_filepath);
                        assert!(cfg_filepath.is_none());
                        if params.fail_allowed_dirs_path_computing {
                            Err(Error::IO(io::Error::from(io::ErrorKind::PermissionDenied)))
                        } else {
                            Ok(filepath.clone())
                        }
                    }
                });
                let renderer = StubRenderer::new().with_stub_of_render_recursively({
                    let tpl_repo_path = ctx.tpl_repo_path.clone();
                    let tpl = params.args.tpl.clone();
                    let expected_vars = ctx.vars.clone();
                    move |_, tpl_dirpath, dest, vars| {
                        assert_eq!(tpl_dirpath, tpl_repo_path.join(&tpl));
                        assert_eq!(dest, expected.dest);
                        assert_eq!(vars, expected_vars);
                        if params.fail_rendering {
                            Err(Error::Liquid {
                                cause: liquid::Error::with_msg("error"),
                                src: LiquidErrorSource::Filename("test".into()),
                            })
                        } else {
                            if let Some(ref content) = params.gitignore_content {
                                write(dest.join(GITIGNORE_FILENAME), content).unwrap();
                            }
                            Ok(())
                        }
                    }
                });
                let vars_loader = StubVariablesLoader::new().with_stub_of_load({
                    let expected_desc = params.args.desc.clone();
                    let expected_overrides = params.args.vars.clone();
                    let vars = ctx.vars.clone();
                    move |_, name, desc, overrides| {
                        assert_eq!(name, ctx.name);
                        assert_eq!(desc, expected_desc);
                        assert_eq!(overrides, expected_overrides);
                        vars.clone()
                    }
                });
                let cmd = NewCommand {
                    args: params.args,
                    fs: Box::new(fs),
                    git: Box::new(git),
                    paths: Box::new(paths),
                    renderer: Box::new(renderer),
                    vars_loader: Box::new(vars_loader),
                };
                let res = cmd.run();
                assert_fn(&ctx, res);
            }
        }
    }

    mod default_variables_loader {
        use super::*;

        mod load {
            use super::*;

            struct Context {
                git_email: &'static str,
                git_username: &'static str,
                name: &'static str,
            }

            struct Parameters {
                desc: Option<String>,
                fail_git_username_retrieving: bool,
                fail_git_email_retrieving: bool,
                overrides: Vec<(String, String)>,
            }

            #[test]
            fn vars_when_git_username_retrieving_failed() {
                test(
                    |_| Parameters {
                        desc: None,
                        fail_git_email_retrieving: false,
                        fail_git_username_retrieving: true,
                        overrides: vec![],
                    },
                    |ctx, vars| {
                        let expected_vars: HashMap<String, String> = HashMap::from_iter([
                            (NAME_VAR_NAME.into(), ctx.name.into()),
                            (GIT_USER_EMAIL_VAR_KEY.into(), ctx.git_email.into()),
                        ]);
                        assert_eq!(vars, expected_vars);
                    },
                )
            }

            #[test]
            fn vars_when_git_email_retrieving_failed() {
                test(
                    |_| Parameters {
                        desc: None,
                        fail_git_email_retrieving: true,
                        fail_git_username_retrieving: false,
                        overrides: vec![],
                    },
                    |ctx, vars| {
                        let expected_vars: HashMap<String, String> = HashMap::from_iter([
                            (NAME_VAR_NAME.into(), ctx.name.into()),
                            (GIT_USER_NAME_VAR_KEY.into(), ctx.git_username.into()),
                        ]);
                        assert_eq!(vars, expected_vars);
                    },
                )
            }

            #[test]
            fn vars_when_custom_args() {
                let name_fn = |ctx: &Context| -> String { format!("{}2", ctx.name) };
                let desc = "description";
                let var_key = "key";
                let var_val = "val";
                test(
                    |ctx| Parameters {
                        desc: Some(desc.into()),
                        fail_git_email_retrieving: false,
                        fail_git_username_retrieving: false,
                        overrides: vec![
                            (NAME_VAR_NAME.into(), name_fn(ctx)),
                            (var_key.into(), var_val.into()),
                        ],
                    },
                    |ctx, vars| {
                        let expected_vars: HashMap<String, String> = HashMap::from_iter([
                            (NAME_VAR_NAME.into(), name_fn(ctx)),
                            (DESCRIPTION_VAR_KEY.into(), desc.into()),
                            (GIT_USER_NAME_VAR_KEY.into(), ctx.git_username.into()),
                            (GIT_USER_EMAIL_VAR_KEY.into(), ctx.git_email.into()),
                            (var_key.into(), var_val.into()),
                        ]);
                        assert_eq!(vars, expected_vars);
                    },
                )
            }

            fn test<P: Fn(&Context) -> Parameters, A: Fn(&Context, HashMap<String, String>)>(
                create_params_fn: P,
                assert_fn: A,
            ) {
                let ctx = Context {
                    git_email: "user@test",
                    git_username: "test",
                    name: "my-project",
                };
                let params = create_params_fn(&ctx);
                let git = StubGit::new().with_stub_of_default_config_value(move |i, key| {
                    if i == 0 {
                        assert_eq!(key, GIT_USER_NAME_CONFIG_KEY);
                        if params.fail_git_username_retrieving {
                            Err(Error::IO(io::Error::from(io::ErrorKind::PermissionDenied)))
                        } else {
                            Ok(ctx.git_username.into())
                        }
                    } else if i == 1 {
                        assert_eq!(key, GIT_USER_EMAIL_CONFIG_KEY);
                        if params.fail_git_email_retrieving {
                            Err(Error::IO(io::Error::from(io::ErrorKind::PermissionDenied)))
                        } else {
                            Ok(ctx.git_email.into())
                        }
                    } else {
                        panic!("unexpected call of default_config_value");
                    }
                });
                let loader = DefaultVariablesLoader { git: Box::new(git) };
                let vars = loader.load(ctx.name.into(), params.desc, params.overrides);
                assert_fn(&ctx, vars);
            }
        }
    }
}
