use std::{
    env::current_dir,
    fs::{copy, create_dir_all, read_to_string, remove_dir_all, DirEntry, File, OpenOptions},
    io,
    path::{Path, PathBuf},
};

use home::home_dir;
use log::{debug, trace};
#[cfg(test)]
use stub_trait::stub;
use tempfile::tempdir;

use crate::err::{Error, Result};

pub type DirEntries = dyn Iterator<Item = io::Result<DirEntry>>;

#[cfg_attr(test, stub)]
pub trait FileSystem {
    fn copy(&self, src: &Path, dest: &Path) -> Result<()>;

    fn create_dir(&self, path: &Path) -> Result<()>;

    fn create_temp_dir(&self) -> Result<PathBuf>;

    fn cwd(&self) -> Result<PathBuf>;

    fn delete_dir(&self, path: &Path) -> Result<()>;

    fn home_dirpath(&self) -> Result<PathBuf>;

    fn open(&self, path: &Path, opts: OpenOptions) -> Result<File>;

    fn read_dir(&self, path: &Path) -> Result<Box<DirEntries>>;

    fn read_to_string(&self, path: &Path) -> Result<String>;
}

pub struct DefaultFileSystem;

impl FileSystem for DefaultFileSystem {
    fn copy(&self, src: &Path, dest: &Path) -> Result<()> {
        debug!("Copying {} into {}", src.display(), dest.display());
        copy(src, dest)
            .map(|len| trace!("{} bytes copied", len))
            .map_err(|err| {
                Error::IO(io::Error::new(
                    err.kind(),
                    format!(
                        "Unable to copy {} into {}: {}",
                        src.display(),
                        dest.display(),
                        err
                    ),
                ))
            })
    }

    fn create_dir(&self, path: &Path) -> Result<()> {
        debug!("Creating directory {}", path.display());
        create_dir_all(path).map_err(|err| {
            Error::IO(io::Error::new(
                err.kind(),
                format!("Unable to create directory {}: {}", path.display(), err),
            ))
        })
    }

    fn create_temp_dir(&self) -> Result<PathBuf> {
        trace!("Creating temporary directory");
        tempdir()
            .map(|temp_dir| temp_dir.into_path())
            .map_err(|err| {
                Error::IO(io::Error::new(
                    err.kind(),
                    format!("Unable to create temporary directory: {}", err),
                ))
            })
    }

    fn cwd(&self) -> Result<PathBuf> {
        trace!("Getting current working directory");
        current_dir().map_err(|err| {
            Error::IO(io::Error::new(
                err.kind(),
                format!("Unable to get current working directory: {}", err),
            ))
        })
    }

    fn delete_dir(&self, path: &Path) -> Result<()> {
        debug!("Deleting directory {}", path.display());
        remove_dir_all(path).map_err(|err| {
            Error::IO(io::Error::new(
                err.kind(),
                format!("Unable to delete directory {}: {}", path.display(), err),
            ))
        })
    }

    fn home_dirpath(&self) -> Result<PathBuf> {
        trace!("Getting home directory");
        home_dir().ok_or(Error::HomeNotFound)
    }

    fn open(&self, path: &Path, opts: OpenOptions) -> Result<File> {
        trace!("Opening file {}", path.display());
        opts.open(path).map_err(|err| {
            Error::IO(io::Error::new(
                err.kind(),
                format!("Unable to open {}: {}", path.display(), err),
            ))
        })
    }

    fn read_dir(&self, path: &Path) -> Result<Box<DirEntries>> {
        trace!("Reading directory {}", path.display());
        path.read_dir()
            .map(|it| Box::new(it) as Box<DirEntries>)
            .map_err(|err| {
                Error::IO(io::Error::new(
                    err.kind(),
                    format!("Unable to read directory {}: {}", path.display(), err),
                ))
            })
    }

    fn read_to_string(&self, path: &Path) -> Result<String> {
        debug!("Reading file {}", path.display());
        read_to_string(path).map_err(|err| {
            Error::IO(io::Error::new(
                err.kind(),
                format!("Unable to read {}: {}", path.display(), err),
            ))
        })
    }
}
