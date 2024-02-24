use std::{collections::HashMap, path::PathBuf};

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::cmd::{OPT_GIT, OPT_TPL};

pub type ProjectctlResult<T = ()> = Result<T, ProjectctlError>;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Context {
    pub env: HashMap<String, String>,
    pub git: HashMap<String, String>,
    pub metadata: ProjectMetadata,
}

#[derive(Debug, thiserror::Error)]
pub enum ProjectctlError {
    #[error("destination file exists")]
    DestExists,
    #[error("file {0} is not contained by project directory")]
    FileOutsideProject(PathBuf),
    #[error("git error: {0}")]
    Git(
        #[from]
        #[source]
        git2::Error,
    ),
    #[error("HTTP error: {0}")]
    Http(
        #[from]
        #[source]
        reqwest::Error,
    ),
    #[error("invalid project name")]
    InvalidProjectName,
    #[error("invalid UTF-8 string")]
    InvalidUtf8,
    #[error("i/o error: {0}")]
    Io(
        #[from]
        #[source]
        std::io::Error,
    ),
    #[error("JSON error: {0}")]
    Json(
        #[from]
        #[source]
        serde_json::Error,
    ),
    #[error("{0}")]
    Liquid(
        #[from]
        #[source]
        liquid::Error,
    ),
    #[error("--{OPT_TPL} must be defined if --{OPT_GIT} is used")]
    MissingTemplate,
    #[error("repository doesn't have any branch")]
    NoDefaultBranch,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum GitRevision {
    Branch(String),
    DefaultBranch,
    Tag(String),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Project {
    pub metadata: ProjectMetadata,
    pub path: PathBuf,
    pub rendered: HashMap<PathBuf, ProjectRenderedEntry>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub struct ProjectFile {
    pub checksum: String,
    pub vars: Value,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub struct ProjectMetadata {
    #[serde(rename = "description")]
    pub desc: Option<String>,
    pub name: String,
    #[serde(rename = "repository")]
    pub repo: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub struct ProjectRenderedEntry {
    pub file: ProjectFile,
    #[serde(rename = "template")]
    pub tpl: Template,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum Template {
    Git {
        rev: GitRevision,
        #[serde(rename = "template")]
        tpl: PathBuf,
        url: String,
    },
    Local(PathBuf),
    Url(String),
}
