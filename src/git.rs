use std::path::Path;

use git2::{Error, Repository};
use log::debug;
#[cfg(test)]
use stub_trait::stub;

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
}

pub struct DefaultGit;

impl Git for DefaultGit {
    fn checkout_repository(
        &self,
        url: &str,
        reference: Option<Reference>,
        dest: &Path,
    ) -> Result<Repository, Error> {
        debug!("Cloning {} into {}", url, dest.display());
        let repo = Repository::clone(url, dest)?;
        if let Some(reference) = reference {
            let reference = match reference {
                Reference::Branch(branch) => format!("refs/remotes/origin/{}", branch),
                Reference::Tag(tag) => format!("refs/tags/{}", tag),
            };
            debug!("Settings HEAD to {}", reference);
            repo.set_head(&reference)?;
        }
        Ok(repo)
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

        mod checkout_repository {
            use super::*;

            struct Context {
                branch: &'static str,
                commit_v1_1_id: Oid,
                commit_v2_id: Oid,
                commit_v3_id: Oid,
                tag: &'static str,
            }

            struct Parameters {
                reference: Option<Reference>,
            }

            #[test]
            fn repo_when_ref_is_unset() {
                test(
                    |_| Parameters { reference: None },
                    |ctx, commit_id| {
                        assert_eq!(commit_id, ctx.commit_v3_id);
                    },
                );
            }

            #[test]
            fn repo_when_ref_is_branch() {
                test(
                    |ctx| Parameters {
                        reference: Some(Reference::Branch(ctx.branch.into())),
                    },
                    |ctx, commit_id| {
                        assert_eq!(commit_id, ctx.commit_v1_1_id);
                    },
                );
            }

            #[test]
            fn repo_when_ref_is_tag() {
                test(
                    |ctx| Parameters {
                        reference: Some(Reference::Tag(ctx.tag.into())),
                    },
                    |ctx, commit_id| {
                        assert_eq!(commit_id, ctx.commit_v2_id);
                    },
                );
            }

            #[inline]
            fn write_and_commit<'a>(
                repo: &'a Repository,
                filepath: &Path,
                msg: &str,
                parents: &[&'a Commit],
                ref_to_update: Option<&str>,
            ) -> Commit<'a> {
                let mut file = File::create(filepath).unwrap();
                write!(file, "{}", msg).unwrap();
                drop(file);
                let relative_filepath = filepath
                    .strip_prefix(repo.path().parent().unwrap())
                    .unwrap();
                let mut index = repo.index().unwrap();
                index.add_path(relative_filepath).unwrap();
                let tree_id = index.write_tree().unwrap();
                let tree = repo.find_tree(tree_id).unwrap();
                let sig = Signature::now("test", "test@local").unwrap();
                let commit_id = repo
                    .commit(ref_to_update, &sig, &sig, msg, &tree, parents)
                    .unwrap();
                repo.find_commit(commit_id).unwrap()
            }

            #[inline]
            fn test<G: Fn(&Context) -> Parameters, A: Fn(&Context, Oid)>(given: G, assert: A) {
                let remote_dirpath = tempdir().unwrap().into_path();
                let filepath = remote_dirpath.join("file");
                let branch = "develop";
                let tag = "v2";
                let remote_repo = Repository::init(&remote_dirpath).unwrap();
                let commit_v1 = write_and_commit(&remote_repo, &filepath, "v1", &[], Some("HEAD"));
                let commit_v1_1 =
                    write_and_commit(&remote_repo, &filepath, "v1_1", &[&commit_v1], None);
                let commit_v2 =
                    write_and_commit(&remote_repo, &filepath, "v2", &[&commit_v1], Some("HEAD"));
                let commit_v3 =
                    write_and_commit(&remote_repo, &filepath, "v3", &[&commit_v2], Some("HEAD"));
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
                let params = given(&ctx);
                let url = remote_dirpath.to_str().unwrap();
                let repo = DefaultGit
                    .checkout_repository(url, params.reference, &dest)
                    .unwrap();
                assert(&ctx, repo.head().unwrap().peel_to_commit().unwrap().id());
            }
        }
    }
}
