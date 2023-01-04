use std::path::Path;

use git2::{build::CheckoutBuilder, Config, Repository};
use log::{debug, trace, warn};
#[cfg(test)]
use stub_trait::stub;

use crate::err::{Error, ErrorKind, Result};

#[derive(Debug, Eq, PartialEq)]
pub enum Reference {
    Branch(String),
    Tag(String),
}

#[cfg_attr(test, stub)]
pub trait Git {
    fn checkout_repository(
        &self,
        url: &str,
        reference: Option<Reference>,
        dest: &Path,
    ) -> Result<Repository>;

    fn default_config_value(&self, key: &str) -> Result<String>;

    fn init(&self, path: &Path) -> Result<Repository>;
}

pub struct DefaultGit {
    cfg: Config,
}

impl DefaultGit {
    pub fn new() -> Self {
        debug!("Reading git default configuration");
        Self {
            cfg: Config::open_default().unwrap_or_else(|err| {
                warn!("Unable to read default git configuration: {}", err);
                Config::new().unwrap()
            }),
        }
    }
}

impl Git for DefaultGit {
    fn checkout_repository(
        &self,
        url: &str,
        reference: Option<Reference>,
        dest: &Path,
    ) -> Result<Repository> {
        debug!("Cloning {} into {}", url, dest.display());
        let repo = Repository::clone(url, dest).map_err(|err| Error {
            kind: ErrorKind::Git(err),
            msg: format!("Unable to clone {} into {}", url, dest.display()),
        })?;
        if let Some(reference) = reference {
            let reference = match reference {
                Reference::Branch(branch) => format!("refs/remotes/origin/{}", branch),
                Reference::Tag(tag) => format!("refs/tags/{}", tag),
            };
            debug!("Setting HEAD to {}", reference);
            repo.set_head(&reference).map_err(|err| Error {
                kind: ErrorKind::Git(err),
                msg: format!("Unable to setting HEAD to {}", reference),
            })?;
            trace!("Checking out HEAD");
            repo.checkout_head(Some(CheckoutBuilder::new().force()))
                .map_err(|err| Error {
                    kind: ErrorKind::Git(err),
                    msg: "Unable to checkout HEAD".into(),
                })?;
        }
        Ok(repo)
    }

    fn default_config_value(&self, key: &str) -> Result<String> {
        debug!("Reading `{}` from git default configuration", key);
        self.cfg.get_string(key).map_err(|err| Error {
            kind: ErrorKind::Git(err),
            msg: format!("Unable to get value of git configuration key `{}`", key),
        })
    }

    fn init(&self, path: &Path) -> Result<Repository> {
        debug!("Initializing git repository into {}", path.display());
        Repository::init(path).map_err(|err| Error {
            kind: ErrorKind::Git(err),
            msg: format!(
                "Unable to initialize git repository into {}",
                path.display()
            ),
        })
    }
}

#[cfg(test)]
mod test {
    use std::{
        fs::{read_to_string, write},
        path::PathBuf,
    };

    use git2::{Commit, Signature};
    use tempfile::tempdir;

    use super::*;

    mod default_git {
        use super::*;

        mod checkout_repository {
            use super::*;

            struct Context {
                branch: &'static str,
                file_content: &'static str,
                file_content_branch: &'static str,
                file_content_tag: &'static str,
                filepath: PathBuf,
                remote_filepath: PathBuf,
                remote_repo: Repository,
                tag: &'static str,
            }

            struct Parameters {
                reference: Option<Reference>,
            }

            #[test]
            fn ok_when_ref_is_none() {
                test(
                    |_| Parameters { reference: None },
                    |ctx, res| {
                        res.unwrap();
                        assert_file_content(ctx, ctx.file_content);
                    },
                );
            }

            #[test]
            fn ok_when_ref_is_branch() {
                test(
                    |ctx| Parameters {
                        reference: Some(Reference::Branch(ctx.branch.into())),
                    },
                    |ctx, res| {
                        res.unwrap();
                        assert_file_content(ctx, ctx.file_content_branch);
                    },
                );
            }

            #[test]
            fn ok_when_ref_is_tag() {
                test(
                    |ctx| Parameters {
                        reference: Some(Reference::Tag(ctx.tag.into())),
                    },
                    |ctx, res| {
                        res.unwrap();
                        assert_file_content(ctx, ctx.file_content_tag);
                    },
                );
            }

            fn assert_file_content(ctx: &Context, expected_content: &str) {
                let content = read_to_string(&ctx.filepath).unwrap();
                assert_eq!(content, expected_content);
            }

            fn commit<'a>(
                ctx: &'a Context,
                content: &str,
                parents: &[&'a Commit],
                update_head: bool,
            ) -> Commit<'a> {
                write(&ctx.remote_filepath, content).unwrap();
                let mut index = ctx.remote_repo.index().unwrap();
                let filename = Path::new(ctx.remote_filepath.file_name().unwrap());
                index.add_path(filename).unwrap();
                let tree_id = index.write_tree().unwrap();
                let tree = ctx.remote_repo.find_tree(tree_id).unwrap();
                let sig = Signature::now("test", "test@local").unwrap();
                let commit_id = ctx
                    .remote_repo
                    .commit(
                        update_head.then_some("HEAD"),
                        &sig,
                        &sig,
                        content,
                        &tree,
                        parents,
                    )
                    .unwrap();
                ctx.remote_repo.find_commit(commit_id).unwrap()
            }

            fn test<P: Fn(&Context) -> Parameters, A: Fn(&Context, Result<Repository>)>(
                create_params_fn: P,
                assert_fn: A,
            ) {
                let remote_dirpath = tempdir().unwrap().into_path();
                let dest = tempdir().unwrap().into_path();
                let remote_repo = Repository::init(&remote_dirpath).unwrap();
                let filename = Path::new("file");
                let ctx = Context {
                    branch: "develop",
                    file_content: "v0.2.0-dev",
                    file_content_branch: "v0.1.1-dev",
                    file_content_tag: "v0.1.0",
                    filepath: dest.join(filename),
                    remote_filepath: remote_dirpath.join(filename),
                    remote_repo,
                    tag: "v0.1.0",
                };
                let params = create_params_fn(&ctx);
                let commit_root = commit(&ctx, "v0.1.0-dev", &[], true);
                let commit_branch = commit(&ctx, ctx.file_content_branch, &[&commit_root], false);
                let commit_tag = commit(&ctx, ctx.file_content_tag, &[&commit_root], true);
                commit(&ctx, ctx.file_content, &[&commit_tag], true);
                ctx.remote_repo
                    .checkout_head(Some(CheckoutBuilder::new().force()))
                    .unwrap();
                ctx.remote_repo
                    .branch(ctx.branch, &commit_branch, false)
                    .unwrap();
                ctx.remote_repo
                    .tag(
                        ctx.tag,
                        commit_tag.as_object(),
                        &commit_tag.author(),
                        ctx.file_content_tag,
                        false,
                    )
                    .unwrap();
                let git = DefaultGit {
                    cfg: Config::open_default().unwrap(),
                };
                let res = git.checkout_repository(
                    remote_dirpath.to_str().unwrap(),
                    params.reference,
                    &dest,
                );
                assert_fn(&ctx, res);
            }
        }

        mod init {
            use super::*;

            #[test]
            fn ok() {
                let path = tempdir().unwrap().into_path();
                let git = DefaultGit {
                    cfg: Config::open_default().unwrap(),
                };
                let repo = git.init(&path).unwrap();
                assert!(repo.is_empty().unwrap());
            }
        }
    }
}
