use std::{
    env::current_dir,
    fmt::{self, Debug, Formatter},
    fs::{create_dir_all, remove_dir_all},
    io,
    path::{Path, PathBuf},
};

use log::debug;
use tempfile::{tempdir, TempDir};

use crate::{
    cli::NewCommandArguments,
    err::Error,
    git::{DefaultGit, Git, Reference},
};

use super::{Command, CommandKind, Result};

type CWDFn = dyn Fn() -> io::Result<PathBuf>;
type TempDirFn = dyn Fn() -> io::Result<TempDir>;

pub struct NewCommand {
    args: NewCommandArguments,
    cwd: Box<CWDFn>,
    git: Box<dyn Git>,
    tempdir: Box<TempDirFn>,
}

impl NewCommand {
    pub fn new(args: NewCommandArguments) -> Self {
        Self {
            args,
            cwd: Box::new(current_dir),
            git: Box::new(DefaultGit),
            tempdir: Box::new(tempdir),
        }
    }

    #[inline]
    fn delete_dir(path: &Path) {
        if let Err(err) = remove_dir_all(path) {
            debug!("Unable to delete {}: {}", path.display(), err);
        }
    }

    fn render_files_recursively(_tpl_dirpath: &Path, _dest: &Path) -> Result {
        todo!();
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
                (self.cwd)().map(|cwd| cwd.join(&self.args.name))
            })
            .map_err(Error::IO)?;
        if dest.exists() {
            return Err(Error::DestinationDirectoryAlreadyExists(dest));
        }
        debug!("Creating {} directory", dest.display());
        create_dir_all(&dest).map_err(Error::IO)?;
        debug!("Creating temporary directory");
        let tpl_repo_path = match (self.tempdir)() {
            Ok(temp_dir) => temp_dir.into_path(),
            Err(err) => {
                Self::delete_dir(&dest);
                return Err(Error::IO(err));
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
            .and(Self::render_files_recursively(&tpl_repo_path, &dest));
        Self::delete_dir(&tpl_repo_path);
        if res.is_err() {
            Self::delete_dir(&dest);
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
    use crate::git::StubGit;

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
                    cwd: Box::new(current_dir),
                    git: Box::new(StubGit::new()),
                    tempdir: Box::new(tempdir),
                };
                match cmd.kind() {
                    CommandKind::New(_) => (),
                }
            }
        }
    }
}
