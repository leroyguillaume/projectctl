use std::{
    borrow::Cow,
    collections::HashMap,
    fs::{create_dir_all, File},
    path::{Path, PathBuf},
};

use git2::{AutotagOption, BranchType, Config, ConfigEntry, FetchOptions, Repository};
use serde::{Deserialize, Serialize};
use tempdir::TempDir;
use tracing::{debug, debug_span, field, warn};

use crate::{
    digest::hash,
    model::{
        GitRevision, Project, ProjectMetadata, ProjectRenderedEntry, ProjectctlError,
        ProjectctlResult,
    },
};

const REMOTE: &str = "origin";

const PROJECTCTL_DIR_NAME: &str = ".projectctl";
const PROJECT_FILE_NAME: &str = "project.json";
const REPO_DIR_NAME: &str = "repositories";
const TMP_DIR_PREFIX: &str = env!("CARGO_PKG_NAME");

#[cfg_attr(test, mockall::automock)]
pub trait FileSystem {
    fn checkout_file(&self, url: &str, path: &Path, rev: &GitRevision)
        -> ProjectctlResult<PathBuf>;

    fn download_file(&self, url: &str) -> ProjectctlResult<PathBuf>;

    fn load_git_config(&self) -> ProjectctlResult<HashMap<String, String>>;

    fn load_project(&self) -> ProjectctlResult<Project>;

    fn project_dir(&self) -> &Path;

    fn projectctl_dir(&self) -> &Path;

    fn save_project(&self, project: &Project) -> ProjectctlResult;
}

pub struct DefaultFileSystem {
    project_dir: PathBuf,
    projectctl_dir: PathBuf,
    tmp_dir: TempDir,
}

impl DefaultFileSystem {
    pub fn init(projectctl_dir: PathBuf, project_dir: PathBuf) -> ProjectctlResult<Self> {
        debug!("creating temporary directory");
        let tmp_dir = TempDir::new(TMP_DIR_PREFIX)?;
        Ok(Self {
            project_dir,
            projectctl_dir,
            tmp_dir,
        })
    }

    fn read_git_config_entry(
        entry: Result<&ConfigEntry, git2::Error>,
        config: &Config,
    ) -> ProjectctlResult<(String, String)> {
        let entry = entry?;
        entry
            .name()
            .ok_or(ProjectctlError::InvalidUtf8)
            .and_then(|name| {
                config
                    .get_string(name)
                    .map(|value| (name.replace('.', "_"), value))
                    .map_err(ProjectctlError::from)
            })
    }

    fn read_git_origin_url<P: AsRef<Path>>(path: P) -> ProjectctlResult<String> {
        debug!(path = %path.as_ref().display(), "open git repository");
        let repo = Repository::open(path)?;
        debug!(remote = REMOTE, "getting remote");
        let remote = repo.find_remote(REMOTE)?;
        debug!("getting remote URL");
        let url = remote.url().ok_or(ProjectctlError::InvalidUtf8)?;
        Ok(url.into())
    }
}

impl FileSystem for DefaultFileSystem {
    fn checkout_file(
        &self,
        url: &str,
        path: &Path,
        rev: &GitRevision,
    ) -> ProjectctlResult<PathBuf> {
        let span = debug_span!("checkout_file", file = %path.display(), refspec = field::Empty, remote = REMOTE, url);
        let _enter = span.enter();
        let dir_name = hash(url);
        let root_path = self.projectctl_dir.join(REPO_DIR_NAME);
        let repo_path = root_path.join(dir_name);
        let repo = if repo_path.is_dir() {
            debug!(path = %repo_path.display(), "opening git repository");
            Repository::open(&repo_path)
        } else {
            ensure_dir_is_created(&root_path)?;
            debug!(path = %repo_path.display(), "cloning git repository");
            Repository::clone(url, &repo_path)
        }?;
        debug!("getting remote");
        let mut remote = repo.find_remote(REMOTE)?;
        let mut opts = FetchOptions::new();
        opts.download_tags(AutotagOption::All)
            .update_fetchhead(false);
        let refspec = match rev {
            GitRevision::Branch(branch) => format!("refs/remotes/{REMOTE}/{branch}"),
            GitRevision::DefaultBranch => {
                debug!("getting local branches");
                let mut branches = repo.branches(Some(BranchType::Local))?;
                debug!("getting default branch");
                let (branch, _) = branches.next().ok_or(ProjectctlError::NoDefaultBranch)??;
                let branch_name = branch.name()?.ok_or(ProjectctlError::InvalidUtf8)?;
                format!("refs/remotes/{REMOTE}/{branch_name}")
            }
            GitRevision::Tag(tag) => format!("refs/tags/{tag}"),
        };
        span.record("refspec", &refspec);
        debug!("fetching remote");
        remote.fetch(&[&refspec], Some(&mut opts), None)?;
        debug!("getting revision");
        let obj = repo.revparse_single(&refspec)?;
        debug!("checking out revision");
        repo.checkout_tree(&obj, None)?;
        debug!("setting head");
        repo.set_head(&refspec)?;
        Ok(repo_path.join(path))
    }

    fn download_file(&self, url: &str) -> ProjectctlResult<PathBuf> {
        let span = debug_span!("download_file", %url);
        let _enter = span.enter();
        let file_name = hash(url);
        let path = self.tmp_dir.path().join(file_name);
        debug!(path = %path.display(), "creating file");
        let mut file = File::create(&path)?;
        debug!(path = %path.display(), "downloading file");
        let mut resp = reqwest::blocking::get(url)?;
        let size = resp.copy_to(&mut file)?;
        debug!(path = %path.display(), size, "file downloaded");
        Ok(path)
    }

    fn load_git_config(&self) -> ProjectctlResult<HashMap<String, String>> {
        let span = debug_span!("git_config");
        let _enter = span.enter();
        debug!(path = %self.project_dir.display(), "trying to open directory as git repository");
        let config = match Repository::open(&self.project_dir) {
            Ok(repo) => {
                debug!("reading repository configuration");
                repo.config()
            }
            Err(err) => {
                debug!(details = %err, "directory doesn't appear to be a repository, loading global git configuration");
                Config::open_default()
            }
        }?;
        debug!("reading configuration");
        let mut config_map = HashMap::new();
        let mut entries = config.entries(None)?;
        while let Some(entry) = entries.next() {
            match Self::read_git_config_entry(entry, &config) {
                Ok((name, value)) => {
                    debug!(name, value, "configuration entry loaded");
                    config_map.insert(name, value);
                }
                Err(err) => {
                    warn!(details = %err, "failed to read git configuration entry, it will be ignored");
                }
            }
        }
        Ok(config_map)
    }

    fn load_project(&self) -> ProjectctlResult<Project> {
        let span = debug_span!("load_project");
        let _enter = span.enter();
        let projectctl_dir = self.project_dir.join(PROJECTCTL_DIR_NAME);
        let project_file_path = projectctl_dir.join(PROJECT_FILE_NAME);
        if project_file_path.is_file() {
            debug!(path = %project_file_path.display(), "opening file");
            let file = File::open(&project_file_path)?;
            debug!("loading project");
            let value: ProjectValue = serde_json::from_reader(&file)?;
            Ok(Project {
                metadata: value.metadata.into_owned(),
                path: self.project_dir.clone(),
                rendered: value.rendered.into_owned(),
            })
        } else {
            debug!(path = %project_file_path.display(), "project file doesn't exist, creating new one");
            debug!(path = %self.project_dir.display(), "reading project name from directory name");
            let name = self
                .project_dir
                .canonicalize()?
                .file_name()
                .ok_or(ProjectctlError::InvalidProjectName)?
                .to_str()
                .ok_or(ProjectctlError::InvalidUtf8)?
                .into();
            let repo = match Self::read_git_origin_url(&self.project_dir) {
                Ok(repo) => Some(repo),
                Err(err) => {
                    debug!(details = %err, "directory doesn't appear to be a repository");
                    None
                }
            };
            Ok(Project {
                metadata: ProjectMetadata {
                    desc: None,
                    name,
                    repo,
                },
                path: self.project_dir.clone(),
                rendered: Default::default(),
            })
        }
    }

    fn project_dir(&self) -> &Path {
        &self.project_dir
    }

    fn projectctl_dir(&self) -> &Path {
        &self.projectctl_dir
    }

    fn save_project(&self, project: &Project) -> ProjectctlResult {
        let span = debug_span!("save_project");
        let _enter = span.enter();
        let projectctl_dir = self.project_dir.join(PROJECTCTL_DIR_NAME);
        let project_file_path = projectctl_dir.join(PROJECT_FILE_NAME);
        ensure_dir_is_created(&projectctl_dir)?;
        debug!(path = %project_file_path.display(), "opening file");
        let mut file = File::create(&project_file_path)?;
        let value = ProjectValue {
            metadata: Cow::Borrowed(&project.metadata),
            rendered: Cow::Borrowed(&project.rendered),
        };
        debug!("writing project file");
        serde_json::to_writer_pretty(&mut file, &value)?;
        Ok(())
    }
}

pub fn canonicalize_path<P: AsRef<Path>>(path: P) -> ProjectctlResult<PathBuf> {
    let path = path.as_ref();
    debug!(path = %path.display(), "canonicalizing path");
    let path = path.canonicalize()?;
    Ok(path)
}

pub fn ensure_dir_is_created<P: AsRef<Path>>(path: P) -> ProjectctlResult {
    let path = path.as_ref();
    if !path.is_dir() {
        debug!(path = %path.display(), "creating directory");
        create_dir_all(path)?;
    }
    Ok(())
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
struct ProjectValue<'a> {
    metadata: Cow<'a, ProjectMetadata>,
    rendered: Cow<'a, HashMap<PathBuf, ProjectRenderedEntry>>,
}

#[cfg(test)]
pub mod test {
    use std::{env::current_dir, fs::write};

    use serde::de::DeserializeOwned;

    use crate::init_tracing;

    use super::*;

    pub fn canonicalize_path<P: AsRef<Path>>(path: P) -> PathBuf {
        super::canonicalize_path(path).expect("failed to canonicalize path")
    }

    pub fn create_empty_file<P: AsRef<Path>>(path: P) -> File {
        if let Some(parent) = path.as_ref().parent() {
            ensure_dir_is_created(parent);
        }
        File::create(path).expect("failed to create file")
    }

    pub fn create_file<P: AsRef<Path>>(path: P, content: &str) {
        if let Some(parent) = path.as_ref().parent() {
            ensure_dir_is_created(parent);
        }
        write(path, content).expect("failed to write into file");
    }

    pub fn create_tmp_dir() -> tempdir::TempDir {
        tempdir::TempDir::new(TMP_DIR_PREFIX).expect("failed to create temporary directory")
    }

    pub fn cwd() -> PathBuf {
        current_dir().expect("failed to get current working directory")
    }

    pub fn download_file<S: AsRef<str>>(url: S) -> String {
        let resp = reqwest::blocking::get(url.as_ref()).expect("failed to download file");
        let body = resp.bytes().expect("failed to read response body");
        String::from_utf8(body.into()).expect("failed to decode UTF-8")
    }

    pub fn ensure_dir_is_created<P: AsRef<Path>>(path: P) {
        super::ensure_dir_is_created(path).expect("failed to create directory");
    }

    pub fn load_json<P: AsRef<Path>, V: DeserializeOwned>(path: P) -> V {
        let file = open_file(path);
        serde_json::from_reader(file).expect("failed to load JSON")
    }

    pub fn open_file<P: AsRef<Path>>(path: P) -> File {
        File::open(path).expect("failed to open file")
    }

    pub fn read_to_string<P: AsRef<Path>>(path: P) -> String {
        std::fs::read_to_string(path).expect("failed to read file")
    }

    pub fn strip_prefix<P: AsRef<Path>>(path: P, prefix: &Path) -> PathBuf {
        path.as_ref()
            .strip_prefix(prefix)
            .expect("failed to strip path prefix")
            .to_path_buf()
    }

    pub fn write_json<P: AsRef<Path>, V: Serialize>(path: P, value: &V) {
        let file = create_empty_file(path);
        serde_json::to_writer(file, value).expect("failed to write JSON");
    }

    mod default_file_system {
        use super::*;

        mod checkout_file {
            use super::*;

            fn run(projectctl_dir: PathBuf, rev: &GitRevision) -> String {
                init_tracing();
                let fs = DefaultFileSystem {
                    project_dir: "project".into(),
                    projectctl_dir,
                    tmp_dir: create_tmp_dir(),
                };
                let path = fs
                    .checkout_file(
                        "https://github.com/leroyguillaume/mockable.git",
                        Path::new("README.md"),
                        rev,
                    )
                    .expect("failed to checkout file");
                read_to_string(path)
            }

            #[test]
            fn when_default_branch() {
                let tmp_dir = create_tmp_dir();
                let rev = GitRevision::DefaultBranch;
                let content = run(tmp_dir.path().to_path_buf(), &rev);
                let expected = test::download_file(
                    "https://raw.githubusercontent.com/leroyguillaume/mockable/main/README.md",
                );
                assert_eq!(content, expected);
            }

            #[test]
            fn when_branch() {
                let tmp_dir = create_tmp_dir();
                let branch = "main";
                let rev = GitRevision::Branch(branch.into());
                let content = run(tmp_dir.path().to_path_buf(), &rev);
                let expected = test::download_file(format!(
                    "https://raw.githubusercontent.com/leroyguillaume/mockable/{branch}/README.md"
                ));
                assert_eq!(content, expected);
            }

            #[test]
            fn when_tag() {
                let tmp_dir = create_tmp_dir();
                let tag = "v0.3.0";
                let rev = GitRevision::Tag(tag.into());
                let content = run(tmp_dir.path().to_path_buf(), &rev);
                let expected = test::download_file(format!(
                    "https://raw.githubusercontent.com/leroyguillaume/mockable/{tag}/README.md"
                ));
                assert_eq!(content, expected);
            }

            #[test]
            fn when_parent_dir_doesnt_exist() {
                let tmp_dir = create_tmp_dir();
                let projectctl_dir = tmp_dir.path().join("projectctl");
                let tag = "v0.3.0";
                let rev = GitRevision::Tag(tag.into());
                let content = run(projectctl_dir, &rev);
                let expected = test::download_file(format!(
                    "https://raw.githubusercontent.com/leroyguillaume/mockable/{tag}/README.md"
                ));
                assert_eq!(content, expected);
            }
        }

        #[test]
        fn download_file() {
            init_tracing();
            let fs = DefaultFileSystem {
                project_dir: "project".into(),
                projectctl_dir: "projectctl".into(),
                tmp_dir: create_tmp_dir(),
            };
            let url = "https://raw.githubusercontent.com/leroyguillaume/mockable/main/README.md";
            let path = fs.download_file(url).expect("failed to download file");
            let content = read_to_string(path);
            let expected = super::download_file(url);
            assert_eq!(content, expected);
        }

        mod load_git_config {
            use super::*;

            fn run(project_dir: PathBuf) {
                init_tracing();
                let fs = DefaultFileSystem {
                    project_dir,
                    projectctl_dir: "projectctl".into(),
                    tmp_dir: create_tmp_dir(),
                };
                fs.load_git_config()
                    .expect("failed to load git configuration");
            }

            #[test]
            fn when_project_dir_is_not_repository() {
                let tmp_dir = create_tmp_dir();
                run(tmp_dir.path().to_path_buf());
            }

            #[test]
            fn when_project_dir_is_repository() {
                run(cwd());
            }
        }

        mod load_project {
            use super::*;

            fn run(project_dir: PathBuf) -> Project {
                init_tracing();
                let fs = DefaultFileSystem {
                    project_dir,
                    projectctl_dir: "projectctl".into(),
                    tmp_dir: create_tmp_dir(),
                };
                fs.load_project().expect("failed to load project")
            }

            #[test]
            fn when_project_doesnt_exist_and_not_a_repository() {
                let name = "a";
                let tmp_dir = create_tmp_dir();
                let project_dir = tmp_dir.path().join(name).join("b").join("..");
                ensure_dir_is_created(&project_dir);
                let project = run(project_dir.clone());
                let expected = Project {
                    metadata: ProjectMetadata {
                        desc: None,
                        name: name.into(),
                        repo: None,
                    },
                    path: project_dir,
                    rendered: Default::default(),
                };
                assert_eq!(project, expected);
            }

            #[test]
            fn when_project_doesnt_exist_and_repository() {
                let url = "https://github.com/leroyguillaume/projectctl.git";
                let name = "projectctl";
                let tmp_dir = create_tmp_dir();
                let project_dir = tmp_dir.path().join(name);
                Repository::clone(url, &project_dir).expect("failed to clone repository");
                let project = run(project_dir.clone());
                let expected = Project {
                    metadata: ProjectMetadata {
                        desc: None,
                        name: name.into(),
                        repo: Some(url.into()),
                    },
                    path: project_dir,
                    rendered: Default::default(),
                };
                assert_eq!(project, expected);
            }

            #[test]
            fn when_project_exists() {
                let tmp_dir = create_tmp_dir();
                let project_file_path = tmp_dir
                    .path()
                    .join(PROJECTCTL_DIR_NAME)
                    .join(PROJECT_FILE_NAME);
                let expected = Project {
                    metadata: ProjectMetadata {
                        desc: Some("desc".into()),
                        name: "name".into(),
                        repo: None,
                    },
                    path: tmp_dir.path().to_path_buf(),
                    rendered: Default::default(),
                };
                let value = ProjectValue {
                    metadata: Cow::Borrowed(&expected.metadata),
                    rendered: Cow::Borrowed(&expected.rendered),
                };
                write_json(project_file_path, &value);
                let project = run(tmp_dir.path().to_path_buf());
                assert_eq!(project, expected);
            }
        }

        mod save_project {
            use super::*;

            fn run(project_dir: PathBuf) {
                init_tracing();
                let project_file_path = project_dir
                    .join(PROJECTCTL_DIR_NAME)
                    .join(PROJECT_FILE_NAME);
                let project = Project {
                    metadata: ProjectMetadata {
                        desc: None,
                        name: "name".into(),
                        repo: None,
                    },
                    path: project_dir.clone(),
                    rendered: Default::default(),
                };
                let fs = DefaultFileSystem {
                    project_dir,
                    projectctl_dir: "projectctl".into(),
                    tmp_dir: create_tmp_dir(),
                };
                fs.save_project(&project).expect("failed to save project");
                let value: ProjectValue = load_json(project_file_path);
                let expected = ProjectValue {
                    metadata: Cow::Owned(project.metadata),
                    rendered: Cow::Owned(project.rendered),
                };
                assert_eq!(value, expected);
            }

            #[test]
            fn when_project_file_doesnt_exist() {
                let tmp_dir = create_tmp_dir();
                run(tmp_dir.path().to_path_buf());
            }

            #[test]
            fn when_project_file_exists() {
                let tmp_dir = create_tmp_dir();
                let project_file_path = tmp_dir
                    .path()
                    .join(PROJECTCTL_DIR_NAME)
                    .join(PROJECT_FILE_NAME);
                create_empty_file(project_file_path);
                run(tmp_dir.path().to_path_buf());
            }
        }
    }
}
