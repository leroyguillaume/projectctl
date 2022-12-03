use std::path::Path;

use git2::{build::CheckoutBuilder, Config, Repository};
use log::{debug, trace, warn};
#[cfg(test)]
use stub_trait::stub;

use crate::err::Error;

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
    ) -> Result<Repository, Error>;

    fn default_config_value(&self, key: &str) -> Result<String, Error>;

    fn init(&self, path: &Path) -> Result<Repository, Error>;
}

pub struct DefaultGit {
    cfg: Config,
}

impl DefaultGit {
    pub fn new() -> Self {
        debug!("Reading git default configuration");
        Self {
            cfg: Config::open_default().unwrap_or_else(|err| {
                warn!("{}", Error::Git(err));
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
    ) -> Result<Repository, Error> {
        debug!("Cloning {} into {}", url, dest.display());
        let repo = Repository::clone(url, dest).map_err(Error::Git)?;
        if let Some(reference) = reference {
            let reference = match reference {
                Reference::Branch(branch) => format!("refs/remotes/origin/{}", branch),
                Reference::Tag(tag) => format!("refs/tags/{}", tag),
            };
            debug!("Setting HEAD to {}", reference);
            repo.set_head(&reference).map_err(Error::Git)?;
            trace!("Checking out HEAD");
            repo.checkout_head(Some(CheckoutBuilder::new().force()))
                .map_err(Error::Git)?;
        }
        Ok(repo)
    }

    fn default_config_value(&self, key: &str) -> Result<String, Error> {
        debug!("Reading `{}` from git default configuration", key);
        self.cfg.get_string(key).map_err(Error::Git)
    }

    fn init(&self, path: &Path) -> Result<Repository, Error> {
        debug!("Initializing git repository into {}", path.display());
        Repository::init(path).map_err(Error::Git)
    }
}

#[cfg(test)]
mod test {
    use std::{fs::File, io::Write};

    use git2::{Commit, Oid, Signature};
    use tempfile::tempdir;

    use super::*;

    mod default_git {
        use super::*;

        mod new {
            use super::*;

            #[test]
            fn git() {
                DefaultGit::new();
            }
        }

        mod checkout_repository {
            use super::*;

            struct Context {
                branch: &'static str,
                commit_v1_1_id: Oid,
                commit_v2_id: Oid,
                commit_v3_id: Oid,
                tag: &'static str,
            }

            #[test]
            fn ok_when_ref_is_unset() {
                test(
                    |_| None,
                    |ctx, commit_id| {
                        assert_eq!(commit_id, ctx.commit_v3_id);
                    },
                );
            }

            #[test]
            fn ok_when_ref_is_branch() {
                test(
                    |ctx| Some(Reference::Branch(ctx.branch.into())),
                    |ctx, commit_id| {
                        assert_eq!(commit_id, ctx.commit_v1_1_id);
                    },
                );
            }

            #[test]
            fn ok_when_ref_is_tag() {
                test(
                    |ctx| Some(Reference::Tag(ctx.tag.into())),
                    |ctx, commit_id| {
                        assert_eq!(commit_id, ctx.commit_v2_id);
                    },
                );
            }

            #[inline]
            fn write_and_commit<'a>(
                repo: &'a Repository,
                repo_dirpath: &Path,
                rel_filepath: &Path,
                msg: &str,
                parents: &[&'a Commit],
                ref_to_update: Option<&str>,
            ) -> Commit<'a> {
                let mut file = File::create(repo_dirpath.join(rel_filepath)).unwrap();
                write!(file, "{}", msg).unwrap();
                drop(file);
                let mut index = repo.index().unwrap();
                index.add_path(rel_filepath).unwrap();
                let tree_id = index.write_tree().unwrap();
                let tree = repo.find_tree(tree_id).unwrap();
                let sig = Signature::now("test", "test@local").unwrap();
                let commit_id = repo
                    .commit(ref_to_update, &sig, &sig, msg, &tree, parents)
                    .unwrap();
                repo.find_commit(commit_id).unwrap()
            }

            #[inline]
            fn test<G: Fn(&Context) -> Option<Reference>, A: Fn(&Context, Oid)>(
                data_from_fn: G,
                assert_fn: A,
            ) {
                let remote_dirpath = tempdir().unwrap().into_path();
                let rel_filepath = Path::new("file");
                let branch = "develop";
                let tag = "v2";
                let remote_repo = Repository::init(&remote_dirpath).unwrap();
                let commit_v1 = write_and_commit(
                    &remote_repo,
                    &remote_dirpath,
                    rel_filepath,
                    "v1",
                    &[],
                    Some("HEAD"),
                );
                let commit_v1_1 = write_and_commit(
                    &remote_repo,
                    &remote_dirpath,
                    rel_filepath,
                    "v1_1",
                    &[&commit_v1],
                    None,
                );
                let commit_v2 = write_and_commit(
                    &remote_repo,
                    &remote_dirpath,
                    rel_filepath,
                    "v2",
                    &[&commit_v1],
                    Some("HEAD"),
                );
                let commit_v3 = write_and_commit(
                    &remote_repo,
                    &remote_dirpath,
                    rel_filepath,
                    "v3",
                    &[&commit_v2],
                    Some("HEAD"),
                );
                remote_repo.branch(branch, &commit_v1_1, false).unwrap();
                remote_repo
                    .tag(tag, commit_v2.as_object(), &commit_v1.author(), "v2", false)
                    .unwrap();
                let dest = tempdir().unwrap().into_path();
                let ctx = Context {
                    branch,
                    commit_v1_1_id: commit_v1_1.id(),
                    commit_v2_id: commit_v2.id(),
                    commit_v3_id: commit_v3.id(),
                    tag,
                };
                let reference = data_from_fn(&ctx);
                let url = remote_dirpath.to_str().unwrap();
                let git = DefaultGit {
                    cfg: Config::open_default().unwrap(),
                };
                let repo = git.checkout_repository(url, reference, &dest).unwrap();
                assert_fn(&ctx, repo.head().unwrap().peel_to_commit().unwrap().id());
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
