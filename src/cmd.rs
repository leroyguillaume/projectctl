use std::{
    fmt::{Display, Formatter},
    fs::File,
    ops::Deref,
    path::PathBuf,
    str::FromStr,
};

use clap::{Parser, Subcommand};
use mockable::{DefaultEnv, Env};
use serde_json::Value;
use tracing::{debug, warn};

use crate::{
    digest::hash_file,
    fs::{canonicalize_path, ensure_dir_is_created, FileSystem},
    model::{
        Context, GitRevision, ProjectRenderedEntry, ProjectctlError, ProjectctlResult, Template,
    },
    render::{LiquidRenderer, Renderer},
};

pub const OPT_GIT: &str = "git";
pub const OPT_TPL: &str = "template";

#[derive(Clone, Debug, Eq, Parser, PartialEq)]
#[command(version)]
pub struct Args {
    #[command(subcommand)]
    pub cmd: Command,
    #[arg(
        long,
        long_help = "Path to working directory",
        env = "PROJECTCTL_PROJECT_DIR",
        default_value = "."
    )]
    pub project_dir: PathBuf,
    #[arg(
        long,
        long_help = "Path to projectctl root directory",
        env = "PROJECTCTL_ROOT_DIR",
        default_value = "~/.projectctl"
    )]
    pub projectctl_dir: PathBuf,
}

#[derive(Clone, Debug, Eq, PartialEq, Subcommand)]
pub enum Command {
    #[command(long_about = "Render a template")]
    Render(RenderCommandArgs),
    #[command(long_about = "Update managed files")]
    Update(UpdateCommandArgs),
}

#[derive(clap::Args, Clone, Debug, Eq, PartialEq)]
pub struct RenderCommandArgs {
    #[arg(long_help = "Path to destination file")]
    pub dest: PathBuf,
    #[arg(short, long, long_help = "Overwrite destination if it exists")]
    pub force: bool,
    #[command(flatten)]
    pub kind: TemplateKindArg,
    #[command(flatten)]
    pub rev: GitRevisionArgs,
    #[arg(short, long = OPT_TPL, long_help = "Path to template\nRequired if template is local file or in git repository\nIf the template is local file, it must target a file contained by the project directory", name = "TEMPLATE")]
    pub tpl: Option<PathBuf>,
    #[arg(long, long_help = "JSON-encoded custom variables", default_value_t = VarsArg(Value::Null))]
    pub vars: VarsArg,
}

#[derive(clap::Args, Clone, Debug, Eq, PartialEq)]
pub struct UpdateCommandArgs {
    #[arg(
        short,
        long,
        long_help = "Overwrite destinations if they changed since last rendering"
    )]
    pub force: bool,
}

#[derive(clap::Args, Clone, Debug, Eq, PartialEq)]
#[group(multiple = false, required = true)]
pub struct TemplateKindArg {
    #[arg(long = OPT_GIT, long_help = "URL to git repository")]
    pub git: Option<String>,
    #[arg(long, long_help = "Use local file path as template")]
    pub local: bool,
    #[arg(long, long_help = "URL to template")]
    pub url: Option<String>,
}

#[derive(clap::Args, Clone, Debug, Default, Eq, PartialEq)]
#[group(multiple = false)]
pub struct GitRevisionArgs {
    #[arg(long = "git-branch", long_help = "Name of the branch to checkout")]
    pub branch: Option<String>,
    #[arg(long = "git-tag", long_help = "Name of the tag to checkout")]
    pub tag: Option<String>,
}

impl From<GitRevisionArgs> for GitRevision {
    fn from(args: GitRevisionArgs) -> Self {
        if let Some(tag) = args.tag {
            Self::Tag(tag)
        } else if let Some(branch) = args.branch {
            Self::Branch(branch)
        } else {
            Self::DefaultBranch
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct VarsArg(Value);

impl AsRef<Value> for VarsArg {
    fn as_ref(&self) -> &Value {
        &self.0
    }
}

impl Deref for VarsArg {
    type Target = Value;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl Display for VarsArg {
    fn fmt(&self, f: &mut Formatter) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl FromStr for VarsArg {
    type Err = serde_json::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let value = serde_json::from_str(s)?;
        Ok(Self(value))
    }
}

pub struct CommandRunner<ENV: Env, FS: FileSystem, RENDERER: Renderer> {
    env: ENV,
    fs: FS,
    renderer: RENDERER,
}

impl<FS: FileSystem> CommandRunner<DefaultEnv, FS, LiquidRenderer> {
    pub fn init(fs: FS) -> ProjectctlResult<Self> {
        Ok(Self {
            env: DefaultEnv,
            fs,
            renderer: LiquidRenderer::init()?,
        })
    }
}

impl<ENV: Env, FS: FileSystem, RENDERER: Renderer> CommandRunner<ENV, FS, RENDERER> {
    pub fn run(&self, cmd: Command) -> ProjectctlResult {
        match cmd {
            Command::Render(args) => self.render(args),
            Command::Update(args) => self.update(args),
        }
    }

    fn render(&self, args: RenderCommandArgs) -> ProjectctlResult {
        let mut project = self.fs.load_project()?;
        let ctx = Context {
            env: self.env.all(),
            git: self.fs.load_git_config()?,
            metadata: project.metadata.clone(),
        };
        if !args.force && args.dest.exists() {
            return Err(ProjectctlError::DestExists);
        }
        if let Some(parent) = args.dest.parent() {
            ensure_dir_is_created(parent)?;
        }
        debug!(path = %args.dest.display(), "creating file");
        File::create(&args.dest)?;
        let project_dir = canonicalize_path(self.fs.project_dir())?;
        let dest = canonicalize_path(&args.dest)?;
        debug!("getting relative path of destination file");
        let dest_rel = dest
            .strip_prefix(&project_dir)
            .map_err(|_| ProjectctlError::FileOutsideProject(dest.clone()))?;
        let (tpl, tpl_path) = if let Some(url) = args.kind.git {
            let tpl = args.tpl.ok_or(ProjectctlError::MissingTemplate)?;
            let rev = GitRevision::from(args.rev);
            let tpl_path = self.fs.checkout_file(&url, &tpl, &rev)?;
            let tpl = Template::Git { rev, tpl, url };
            (tpl, tpl_path)
        } else if let Some(url) = args.kind.url {
            let tpl_path = self.fs.download_file(&url)?;
            (Template::Url(url), tpl_path)
        } else if args.kind.local {
            let tpl_path = args.tpl.ok_or(ProjectctlError::MissingTemplate)?;
            let tpl_path = canonicalize_path(tpl_path)?;
            debug!("getting relative path of template file");
            let tpl_path_ref = tpl_path
                .strip_prefix(&project_dir)
                .map_err(|_| ProjectctlError::FileOutsideProject(tpl_path.clone()))?
                .to_path_buf();
            let tpl = Template::Local(tpl_path_ref);
            (tpl, tpl_path)
        } else {
            unreachable!();
        };
        let file = self
            .renderer
            .render(&tpl_path, &args.dest, args.vars.0, &ctx)?;
        let entry = ProjectRenderedEntry { file, tpl };
        project.rendered.insert(dest_rel.to_path_buf(), entry);
        self.fs.save_project(&project)?;
        Ok(())
    }

    fn update(&self, args: UpdateCommandArgs) -> ProjectctlResult {
        let project_dir = self.fs.project_dir();
        let project_init = self.fs.load_project()?;
        let mut project_final = project_init.clone();
        let ctx = Context {
            env: self.env.all(),
            git: self.fs.load_git_config()?,
            metadata: project_init.metadata,
        };
        for (path, entry) in project_init.rendered {
            let path = project_dir.join(path);
            if let Some(parent) = path.parent() {
                ensure_dir_is_created(parent)?;
            }
            if !args.force && path.exists() {
                let checksum = hash_file(&path)?;
                if checksum != entry.file.checksum {
                    warn!(path = %path.display(), "file changed since last rendering, it will be ignored");
                    continue;
                }
            }
            let tpl_path = match &entry.tpl {
                Template::Git { rev, tpl, url } => self.fs.checkout_file(url, tpl, rev)?,
                Template::Local(path) => project_dir.join(path),
                Template::Url(url) => self.fs.download_file(url)?,
            };
            let file = self
                .renderer
                .render(&tpl_path, &path, entry.file.vars, &ctx)?;
            let entry = ProjectRenderedEntry { file, ..entry };
            project_final.rendered.insert(path, entry);
        }
        self.fs.save_project(&project_final)?;
        Ok(())
    }
}

#[cfg(test)]
mod test {
    use std::collections::HashMap;

    use mockable::MockEnv;
    use mockall::predicate::*;
    use tempdir::TempDir;

    use super::*;

    use crate::{
        fs::{self, MockFileSystem},
        model::{Context, Project, ProjectFile, ProjectMetadata},
        render::MockRenderer,
    };

    mod command_runner {
        use super::*;

        mod run {
            use super::*;

            mod render {
                use super::*;

                struct Data {
                    dest: PathBuf,
                    git_tpl: PathBuf,
                    git_url: String,
                    project_dir: PathBuf,
                    rev: GitRevision,
                    tpl_path: PathBuf,
                    url: String,
                }

                impl Data {
                    fn new(tmp_dir: &TempDir) -> Self {
                        let tpl = "examples/templates/CONTRIBUTING.md.liquid";
                        Self {
                            dest: tmp_dir.path().join("dest"),
                            git_tpl: tpl.into(),
                            git_url: "https://github.com/leroyguillaume/projectctl".into(),
                            project_dir: tmp_dir.path().to_path_buf(),
                            rev: GitRevision::DefaultBranch,
                            tpl_path: "tpl".into(),
                            url: format!("https://raw.githubusercontent.com/leroyguillaume/projectctl/main/{tpl}"),
                        }
                    }
                }

                #[derive(Default)]
                struct Mocks {
                    checkout_file: bool,
                    download_file: bool,
                    project_dir: bool,
                    render: bool,
                    save_project: bool,
                }

                fn run(
                    data: Data,
                    args: RenderCommandArgs,
                    tpl: Template,
                    tpl_path: PathBuf,
                    dest: PathBuf,
                    mocks: Mocks,
                ) -> ProjectctlResult {
                    let ctx = Context {
                        env: Default::default(),
                        git: Default::default(),
                        metadata: ProjectMetadata {
                            desc: None,
                            name: "name".into(),
                            repo: None,
                        },
                    };
                    let entry = ProjectRenderedEntry {
                        file: ProjectFile {
                            checksum: "checksum".into(),
                            vars: args.vars.0.clone(),
                        },
                        tpl,
                    };
                    let project_init = Project {
                        metadata: ctx.metadata.clone(),
                        path: "project".into(),
                        rendered: Default::default(),
                    };
                    let project_final = Project {
                        rendered: HashMap::from_iter([(dest, entry.clone())]),
                        ..project_init.clone()
                    };
                    let mut env = MockEnv::new();
                    env.expect_all().times(1).returning({
                        let env = ctx.env.clone();
                        move || env.clone()
                    });
                    let mut fs = MockFileSystem::new();
                    fs.expect_load_git_config().times(1).returning({
                        let git: HashMap<String, String> = ctx.git.clone();
                        move || Ok(git.clone())
                    });
                    fs.expect_checkout_file()
                        .with(
                            eq(data.git_url.clone()),
                            eq(data.git_tpl.clone()),
                            eq(data.rev.clone()),
                        )
                        .times(mocks.checkout_file as usize)
                        .returning({
                            let tpl_path = tpl_path.clone();
                            move |_, _, _| Ok(tpl_path.clone())
                        });
                    fs.expect_download_file()
                        .with(eq(data.url.clone()))
                        .times(mocks.download_file as usize)
                        .returning({
                            let tpl_path = tpl_path.clone();
                            move |_| Ok(tpl_path.clone())
                        });
                    fs.expect_load_project().times(1).returning({
                        let project_init = project_init.clone();
                        move || Ok(project_init.clone())
                    });
                    fs.expect_save_project()
                        .with(eq(project_final))
                        .times(mocks.save_project as usize)
                        .returning(|_| Ok(()));
                    fs.expect_project_dir()
                        .times(mocks.project_dir as usize)
                        .return_const(data.project_dir);
                    let mut renderer = MockRenderer::new();
                    renderer
                        .expect_render()
                        .with(
                            eq(tpl_path),
                            eq(args.dest.clone()),
                            eq(args.vars.0.clone()),
                            eq(ctx),
                        )
                        .times(mocks.render as usize)
                        .returning({
                            let file = entry.file.clone();
                            move |_, _, _, _| Ok(file.clone())
                        });
                    let runner = CommandRunner { env, fs, renderer };
                    runner.run(Command::Render(args))
                }

                #[test]
                fn when_git_with_tpl_undefined() {
                    let tmp_dir = fs::test::create_tmp_dir();
                    let data = Data::new(&tmp_dir);
                    let tpl = Template::Git {
                        rev: data.rev.clone(),
                        tpl: data.git_tpl.clone(),
                        url: data.git_url.clone(),
                    };
                    let args = RenderCommandArgs {
                        dest: data.dest.clone(),
                        force: false,
                        rev: Default::default(),
                        tpl: None,
                        kind: TemplateKindArg {
                            git: Some(data.git_url.clone()),
                            local: false,
                            url: None,
                        },
                        vars: VarsArg(Value::Null),
                    };
                    let dest = fs::test::strip_prefix(&data.dest, tmp_dir.path());
                    let mocks = Mocks {
                        project_dir: true,
                        ..Default::default()
                    };
                    let tpl_path = data.tpl_path.clone();
                    let err =
                        run(data, args, tpl, tpl_path, dest, mocks).expect_err("should failed");
                    assert!(matches!(err, ProjectctlError::MissingTemplate));
                }

                #[test]
                fn when_dest_exists_with_force_disabled() {
                    let tmp_dir = fs::test::create_tmp_dir();
                    let data = Data::new(&tmp_dir);
                    fs::test::create_empty_file(&data.dest);
                    let tpl = Template::Url(data.url.clone());
                    let args = RenderCommandArgs {
                        dest: data.dest.clone(),
                        force: false,
                        kind: TemplateKindArg {
                            git: None,
                            local: false,
                            url: Some(data.url.clone()),
                        },
                        rev: Default::default(),
                        tpl: Some(data.git_tpl.clone()),
                        vars: VarsArg(Value::Null),
                    };
                    let dest = fs::test::strip_prefix(&data.dest, tmp_dir.path());
                    let mocks = Mocks::default();
                    let tpl_path = data.tpl_path.clone();
                    let err =
                        run(data, args, tpl, tpl_path, dest, mocks).expect_err("should failed");
                    assert!(matches!(err, ProjectctlError::DestExists));
                }

                #[test]
                fn when_dest_outside_project_dir() {
                    let tmp_dir = fs::test::create_tmp_dir();
                    let tmp_dir_dest = fs::test::create_tmp_dir();
                    let dest = tmp_dir_dest.path().join("dest");
                    let data = Data::new(&tmp_dir);
                    fs::test::create_empty_file(&data.dest);
                    let tpl = Template::Url(data.url.clone());
                    let args = RenderCommandArgs {
                        dest: dest.clone(),
                        force: false,
                        rev: Default::default(),
                        tpl: Some(data.git_tpl.clone()),
                        kind: TemplateKindArg {
                            git: None,
                            local: false,
                            url: Some(data.url.clone()),
                        },
                        vars: VarsArg(Value::Null),
                    };
                    let mocks = Mocks {
                        project_dir: true,
                        ..Default::default()
                    };
                    let tpl_path = data.tpl_path.clone();
                    let err = run(data, args, tpl, tpl_path, dest.clone(), mocks)
                        .expect_err("should failed");
                    match err {
                        ProjectctlError::FileOutsideProject(path) => {
                            let dest = fs::test::canonicalize_path(dest);
                            assert_eq!(path, dest);
                        }
                        _ => panic!("{err}"),
                    };
                }

                #[test]
                fn when_git_with_default_args() {
                    let tmp_dir = fs::test::create_tmp_dir();
                    let data = Data::new(&tmp_dir);
                    let tpl = Template::Git {
                        rev: data.rev.clone(),
                        tpl: data.git_tpl.clone(),
                        url: data.git_url.clone(),
                    };
                    let args = RenderCommandArgs {
                        dest: data.dest.clone(),
                        force: false,
                        kind: TemplateKindArg {
                            git: Some(data.git_url.clone()),
                            local: false,
                            url: None,
                        },
                        rev: Default::default(),
                        tpl: Some(data.git_tpl.clone()),
                        vars: VarsArg(Value::Null),
                    };
                    let dest = fs::test::strip_prefix(&data.dest, tmp_dir.path());
                    let mocks = Mocks {
                        checkout_file: true,
                        project_dir: true,
                        render: true,
                        save_project: true,
                        ..Default::default()
                    };
                    let tpl_path = data.tpl_path.clone();
                    run(data, args, tpl, tpl_path, dest, mocks).expect("failed to run command");
                }

                #[test]
                fn when_git_with_branch() {
                    let tmp_dir = fs::test::create_tmp_dir();
                    let branch = "branch";
                    let data = Data {
                        rev: GitRevision::Branch(branch.into()),
                        ..Data::new(&tmp_dir)
                    };
                    let tpl = Template::Git {
                        rev: data.rev.clone(),
                        tpl: data.git_tpl.clone(),
                        url: data.git_url.clone(),
                    };
                    let args = RenderCommandArgs {
                        dest: data.dest.clone(),
                        force: false,
                        kind: TemplateKindArg {
                            git: Some(data.git_url.clone()),
                            local: false,
                            url: None,
                        },
                        rev: GitRevisionArgs {
                            branch: Some(branch.into()),
                            tag: None,
                        },
                        tpl: Some(data.git_tpl.clone()),
                        vars: VarsArg(Value::Null),
                    };
                    let dest = fs::test::strip_prefix(&data.dest, tmp_dir.path());
                    let mocks = Mocks {
                        checkout_file: true,
                        project_dir: true,
                        render: true,
                        save_project: true,
                        ..Default::default()
                    };
                    let tpl_path = data.tpl_path.clone();
                    run(data, args, tpl, tpl_path, dest, mocks).expect("failed to run command");
                }

                #[test]
                fn when_git_with_tag() {
                    let tmp_dir = fs::test::create_tmp_dir();
                    let tag = "tag";
                    let data = Data {
                        rev: GitRevision::Tag(tag.into()),
                        ..Data::new(&tmp_dir)
                    };
                    let tpl = Template::Git {
                        rev: data.rev.clone(),
                        tpl: data.git_tpl.clone(),
                        url: data.git_url.clone(),
                    };
                    let args = RenderCommandArgs {
                        dest: data.dest.clone(),
                        force: false,
                        kind: TemplateKindArg {
                            git: Some(data.git_url.clone()),
                            local: false,
                            url: None,
                        },
                        rev: GitRevisionArgs {
                            branch: None,
                            tag: Some(tag.into()),
                        },
                        tpl: Some(data.git_tpl.clone()),
                        vars: VarsArg(Value::Null),
                    };
                    let dest = fs::test::strip_prefix(&data.dest, tmp_dir.path());
                    let mocks = Mocks {
                        checkout_file: true,
                        project_dir: true,
                        render: true,
                        save_project: true,
                        ..Default::default()
                    };
                    let tpl_path = data.tpl_path.clone();
                    run(data, args, tpl, tpl_path, dest, mocks).expect("failed to run command");
                }

                #[test]
                fn when_git_with_overridden_args() {
                    let tmp_dir = fs::test::create_tmp_dir();
                    let data = Data::new(&tmp_dir);
                    fs::test::create_empty_file(&data.dest);
                    let tpl = Template::Git {
                        rev: data.rev.clone(),
                        tpl: data.git_tpl.clone(),
                        url: data.git_url.clone(),
                    };
                    let args = RenderCommandArgs {
                        dest: data.dest.clone(),
                        force: true,
                        kind: TemplateKindArg {
                            git: Some(data.git_url.clone()),
                            local: false,
                            url: None,
                        },
                        rev: Default::default(),
                        tpl: Some(data.git_tpl.clone()),
                        vars: VarsArg(Value::Null),
                    };
                    let dest = fs::test::strip_prefix(&data.dest, tmp_dir.path());
                    let mocks = Mocks {
                        checkout_file: true,
                        project_dir: true,
                        render: true,
                        save_project: true,
                        ..Default::default()
                    };
                    let tpl_path = data.tpl_path.clone();
                    run(data, args, tpl, tpl_path, dest, mocks).expect("failed to run command");
                }

                #[test]
                fn when_url_with_default_args() {
                    let tmp_dir = fs::test::create_tmp_dir();
                    let data = Data::new(&tmp_dir);
                    let tpl = Template::Url(data.url.clone());
                    let args = RenderCommandArgs {
                        dest: data.dest.clone(),
                        force: false,
                        kind: TemplateKindArg {
                            git: None,
                            local: false,
                            url: Some(data.url.clone()),
                        },
                        rev: Default::default(),
                        tpl: None,
                        vars: VarsArg(Value::Null),
                    };
                    let dest = fs::test::strip_prefix(&data.dest, tmp_dir.path());
                    let mocks = Mocks {
                        download_file: true,
                        project_dir: true,
                        render: true,
                        save_project: true,
                        ..Default::default()
                    };
                    let tpl_path = data.tpl_path.clone();
                    run(data, args, tpl, tpl_path, dest, mocks).expect("failed to run command");
                }

                #[test]
                fn when_local_with_tpl_undefined() {
                    let tmp_dir = fs::test::create_tmp_dir();
                    let data = Data::new(&tmp_dir);
                    let tpl = Template::Local(data.tpl_path.clone());
                    let args = RenderCommandArgs {
                        dest: data.dest.clone(),
                        force: false,
                        rev: Default::default(),
                        tpl: None,
                        kind: TemplateKindArg {
                            git: None,
                            local: true,
                            url: None,
                        },
                        vars: VarsArg(Value::Null),
                    };
                    let dest = fs::test::strip_prefix(&data.dest, tmp_dir.path());
                    let mocks = Mocks {
                        project_dir: true,
                        ..Default::default()
                    };
                    let tpl_path = data.project_dir.join(&data.tpl_path);
                    let err =
                        run(data, args, tpl, tpl_path, dest, mocks).expect_err("should failed");
                    assert!(matches!(err, ProjectctlError::MissingTemplate));
                }

                #[test]
                fn when_local_with_tpl_outside_project_dir() {
                    let tmp_dir = fs::test::create_tmp_dir();
                    let tmp_dir_tpl = fs::test::create_tmp_dir();
                    let tpl_path = tmp_dir_tpl.path().join("tpl");
                    fs::test::create_empty_file(&tpl_path);
                    let data = Data::new(&tmp_dir);
                    let tpl = Template::Local(data.tpl_path.clone());
                    let args = RenderCommandArgs {
                        dest: data.dest.clone(),
                        force: false,
                        rev: Default::default(),
                        tpl: Some(tpl_path.clone()),
                        kind: TemplateKindArg {
                            git: None,
                            local: true,
                            url: None,
                        },
                        vars: VarsArg(Value::Null),
                    };
                    let dest = fs::test::strip_prefix(&data.dest, tmp_dir.path());
                    let mocks = Mocks {
                        project_dir: true,
                        ..Default::default()
                    };
                    let err = run(data, args, tpl, tpl_path.clone(), dest, mocks)
                        .expect_err("should failed");
                    match err {
                        ProjectctlError::FileOutsideProject(path) => {
                            let tpl_path = fs::test::canonicalize_path(tpl_path);
                            assert_eq!(path, tpl_path);
                        }
                        _ => panic!("{err}"),
                    };
                }

                #[test]
                fn when_local_with_default_args() {
                    let tmp_dir = fs::test::create_tmp_dir();
                    let tpl_path = tmp_dir.path().join("tpl");
                    fs::test::create_empty_file(&tpl_path);
                    let tpl_path_rel = fs::test::strip_prefix(&tpl_path, tmp_dir.path());
                    let data = Data::new(&tmp_dir);
                    let tpl = Template::Local(tpl_path_rel);
                    let args = RenderCommandArgs {
                        dest: data.dest.clone(),
                        force: false,
                        rev: Default::default(),
                        tpl: Some(tpl_path.clone()),
                        kind: TemplateKindArg {
                            git: None,
                            local: true,
                            url: None,
                        },
                        vars: VarsArg(Value::Null),
                    };
                    let dest = fs::test::strip_prefix(&data.dest, tmp_dir.path());
                    let mocks = Mocks {
                        project_dir: true,
                        render: true,
                        save_project: true,
                        ..Default::default()
                    };
                    let tpl_path = fs::test::canonicalize_path(tpl_path);
                    run(data, args, tpl, tpl_path, dest, mocks).expect("failed to run command");
                }
            }

            mod update {
                use super::*;

                struct Data {
                    checksum_final: String,
                    checksum_init: String,
                    file_content: String,
                    file_path: PathBuf,
                    git_tpl: PathBuf,
                    git_url: String,
                    project_dir: PathBuf,
                    rev: GitRevision,
                    tpl_path: PathBuf,
                    url: String,
                    vars: Value,
                }

                impl Data {
                    fn new(tmp_dir: &TempDir) -> Self {
                        let tpl = "examples/templates/CONTRIBUTING.md.liquid";
                        Self {
                            checksum_final: "checksum_final".into(),
                            checksum_init:
                                "c0535e4be2b79ffd93291305436bf889314e4a3faec05ecffcbb7df31ad9e51a"
                                    .into(),
                            file_content: "Hello world!".into(),
                            file_path: tmp_dir.path().join("dir").join("path"),
                            git_tpl: tpl.into(),
                            git_url: "https://github.com/leroyguillaume/projectctl".into(),
                            project_dir: tmp_dir.path().to_path_buf(),
                            rev: GitRevision::DefaultBranch,
                            tpl_path: "local".into(),
                            url: format!("https://raw.githubusercontent.com/leroyguillaume/projectctl/main/{tpl}"),
                            vars: Value::Null,
                        }
                    }
                }

                #[derive(Default)]
                struct Mocks {
                    checkout_file: bool,
                    download_file: bool,
                    render: bool,
                }

                fn run(
                    data: Data,
                    args: UpdateCommandArgs,
                    entry_init: ProjectRenderedEntry,
                    entry_final: ProjectRenderedEntry,
                    tpl_path: PathBuf,
                    mocks: Mocks,
                ) {
                    let ctx = Context {
                        env: Default::default(),
                        git: Default::default(),
                        metadata: ProjectMetadata {
                            desc: None,
                            name: "name".into(),
                            repo: None,
                        },
                    };
                    let project_init = Project {
                        metadata: ctx.metadata.clone(),
                        path: "project".into(),
                        rendered: HashMap::from_iter([(
                            data.file_path.clone(),
                            entry_init.clone(),
                        )]),
                    };
                    let project_final = Project {
                        rendered: HashMap::from_iter([(
                            data.file_path.clone(),
                            entry_final.clone(),
                        )]),
                        ..project_init.clone()
                    };
                    let mut env = MockEnv::new();
                    env.expect_all().times(1).returning({
                        let env = ctx.env.clone();
                        move || env.clone()
                    });
                    let mut fs = MockFileSystem::new();
                    fs.expect_load_git_config().times(1).returning({
                        let git: HashMap<String, String> = ctx.git.clone();
                        move || Ok(git.clone())
                    });
                    fs.expect_checkout_file()
                        .with(
                            eq(data.git_url.clone()),
                            eq(data.git_tpl.clone()),
                            eq(data.rev.clone()),
                        )
                        .times(mocks.checkout_file as usize)
                        .returning({
                            let tpl_path = tpl_path.clone();
                            move |_, _, _| Ok(tpl_path.clone())
                        });
                    fs.expect_download_file()
                        .with(eq(data.url.clone()))
                        .times(mocks.download_file as usize)
                        .returning({
                            let tpl_path = tpl_path.clone();
                            move |_| Ok(tpl_path.clone())
                        });
                    fs.expect_load_project().times(1).returning({
                        let project_init = project_init.clone();
                        move || Ok(project_init.clone())
                    });
                    fs.expect_save_project()
                        .with(eq(project_final))
                        .times(1)
                        .returning(|_| Ok(()));
                    fs.expect_project_dir()
                        .times(1)
                        .return_const(data.project_dir.clone());
                    let mut renderer = MockRenderer::new();
                    renderer
                        .expect_render()
                        .with(
                            eq(tpl_path),
                            eq(data.file_path.clone()),
                            eq(entry_init.file.vars),
                            eq(ctx),
                        )
                        .times(mocks.render as usize)
                        .returning({
                            let file = entry_final.file.clone();
                            move |_, _, _, _| Ok(file.clone())
                        });
                    let runner = CommandRunner { env, fs, renderer };
                    runner
                        .run(Command::Update(args))
                        .expect("failed to run command");
                }

                #[test]
                fn when_file_changed_and_force_disabled() {
                    let tmp_dir = fs::test::create_tmp_dir();
                    let data = Data::new(&tmp_dir);
                    fs::test::create_file(&data.file_path, &data.file_content);
                    let entry_init = ProjectRenderedEntry {
                        file: ProjectFile {
                            checksum: "".into(),
                            vars: data.vars.clone(),
                        },
                        tpl: Template::Git {
                            rev: data.rev.clone(),
                            tpl: data.git_tpl.clone(),
                            url: data.git_url.clone(),
                        },
                    };
                    let entry_final = entry_init.clone();
                    let args = UpdateCommandArgs { force: false };
                    let mocks = Mocks::default();
                    let tpl_path = data.tpl_path.clone();
                    run(data, args, entry_init, entry_final, tpl_path, mocks);
                }

                #[test]
                fn when_git_with_default_args() {
                    let tmp_dir = fs::test::create_tmp_dir();
                    let data = Data::new(&tmp_dir);
                    fs::test::create_file(&data.file_path, &data.file_content);
                    let entry_init = ProjectRenderedEntry {
                        file: ProjectFile {
                            checksum: data.checksum_init.clone(),
                            vars: data.vars.clone(),
                        },
                        tpl: Template::Git {
                            rev: data.rev.clone(),
                            tpl: data.git_tpl.clone(),
                            url: data.git_url.clone(),
                        },
                    };
                    let entry_final = ProjectRenderedEntry {
                        file: ProjectFile {
                            checksum: data.checksum_final.clone(),
                            ..entry_init.file.clone()
                        },
                        ..entry_init.clone()
                    };
                    let args = UpdateCommandArgs { force: false };
                    let mocks = Mocks {
                        checkout_file: true,
                        render: true,
                        ..Default::default()
                    };
                    let tpl_path = data.tpl_path.clone();
                    run(data, args, entry_init, entry_final, tpl_path, mocks);
                }

                #[test]
                fn when_url_with_default_args() {
                    let tmp_dir = fs::test::create_tmp_dir();
                    let data = Data::new(&tmp_dir);
                    fs::test::create_file(&data.file_path, &data.file_content);
                    let entry_init = ProjectRenderedEntry {
                        file: ProjectFile {
                            checksum: data.checksum_init.clone(),
                            vars: data.vars.clone(),
                        },
                        tpl: Template::Url(data.url.clone()),
                    };
                    let entry_final = ProjectRenderedEntry {
                        file: ProjectFile {
                            checksum: data.checksum_final.clone(),
                            ..entry_init.file.clone()
                        },
                        ..entry_init.clone()
                    };
                    let args = UpdateCommandArgs { force: false };
                    let mocks = Mocks {
                        download_file: true,
                        render: true,
                        ..Default::default()
                    };
                    let tpl_path = data.tpl_path.clone();
                    run(data, args, entry_init, entry_final, tpl_path, mocks);
                }

                #[test]
                fn when_file_doesnt_exist() {
                    let tmp_dir = fs::test::create_tmp_dir();
                    let data = Data::new(&tmp_dir);
                    let entry_init = ProjectRenderedEntry {
                        file: ProjectFile {
                            checksum: data.checksum_init.clone(),
                            vars: data.vars.clone(),
                        },
                        tpl: Template::Url(data.url.clone()),
                    };
                    let entry_final = ProjectRenderedEntry {
                        file: ProjectFile {
                            checksum: data.checksum_final.clone(),
                            ..entry_init.file.clone()
                        },
                        ..entry_init.clone()
                    };
                    let args = UpdateCommandArgs { force: false };
                    let mocks = Mocks {
                        download_file: true,
                        render: true,
                        ..Default::default()
                    };
                    let tpl_path = data.tpl_path.clone();
                    run(data, args, entry_init, entry_final, tpl_path, mocks);
                }

                #[test]
                fn when_local_with_default_args() {
                    let tmp_dir = fs::test::create_tmp_dir();
                    let data = Data::new(&tmp_dir);
                    fs::test::create_file(&data.file_path, &data.file_content);
                    let entry_init = ProjectRenderedEntry {
                        file: ProjectFile {
                            checksum: data.checksum_init.clone(),
                            vars: data.vars.clone(),
                        },
                        tpl: Template::Local(data.tpl_path.clone()),
                    };
                    let entry_final = ProjectRenderedEntry {
                        file: ProjectFile {
                            checksum: data.checksum_final.clone(),
                            ..entry_init.file.clone()
                        },
                        ..entry_init.clone()
                    };
                    let args = UpdateCommandArgs { force: false };
                    let mocks = Mocks {
                        render: true,
                        ..Default::default()
                    };
                    let tpl_path = data.project_dir.join(&data.tpl_path);
                    run(data, args, entry_init, entry_final, tpl_path, mocks);
                }
            }
        }
    }
}
