use std::{
    env::current_dir,
    fs::{copy, create_dir_all, remove_dir_all, DirEntry, File, OpenOptions},
    io,
    path::{Path, PathBuf},
};

use log::{debug, trace};
#[cfg(test)]
use stub_trait::stub;
use tempfile::tempdir;

use crate::err::Error;

pub type DirEntries = dyn Iterator<Item = io::Result<DirEntry>>;
pub type Result<T> = std::result::Result<T, Error>;

#[cfg_attr(test, stub)]
pub trait FileSystem {
    fn copy(&self, src: &Path, dest: &Path) -> Result<()>;

    fn create_dir(&self, path: &Path) -> Result<()>;

    fn create_temp_dir(&self) -> Result<PathBuf>;

    fn cwd(&self) -> Result<PathBuf>;

    fn delete_dir(&self, path: &Path) -> Result<()>;

    fn open(&self, path: &Path, opts: OpenOptions) -> Result<File>;

    fn read_dir(&self, path: &Path) -> Result<Box<DirEntries>>;
}

pub struct DefaultFileSystem;

impl FileSystem for DefaultFileSystem {
    fn copy(&self, src: &Path, dest: &Path) -> Result<()> {
        debug!("Copying {} into {}", src.display(), dest.display());
        copy(src, dest)
            .map(|len| trace!("{} bytes copied", len))
            .map_err(Error::IO)
    }

    fn create_dir(&self, path: &Path) -> Result<()> {
        debug!("Creating directory {}", path.display());
        create_dir_all(path).map_err(Error::IO)
    }

    fn create_temp_dir(&self) -> Result<PathBuf> {
        trace!("Creating temporary directory");
        tempdir()
            .map(|temp_dir| temp_dir.into_path())
            .map_err(Error::IO)
    }

    fn cwd(&self) -> Result<PathBuf> {
        trace!("Getting current working directory");
        current_dir().map_err(Error::IO)
    }

    fn delete_dir(&self, path: &Path) -> Result<()> {
        debug!("Deleting directory {}", path.display());
        remove_dir_all(path).map_err(Error::IO)
    }

    fn open(&self, path: &Path, opts: OpenOptions) -> Result<File> {
        trace!("Opening file {}", path.display());
        opts.open(path).map_err(Error::IO)
    }

    fn read_dir(&self, path: &Path) -> Result<Box<DirEntries>> {
        trace!("Reading directory {}", path.display());
        path.read_dir()
            .map(|it| Box::new(it) as Box<DirEntries>)
            .map_err(Error::IO)
    }
}

#[cfg(test)]
mod test {
    use std::{
        collections::HashSet,
        fs::{read_to_string, File},
        io::Write,
    };

    use super::*;

    mod default_file_system {
        use super::*;

        mod copy {
            use super::*;

            #[test]
            fn ok() {
                let dirpath = tempdir().unwrap().into_path();
                let src_path = dirpath.join("src");
                let mut src_file = File::create(&src_path).unwrap();
                let src_file_content = "Hello world!";
                write!(src_file, "{}", src_file_content).unwrap();
                let dest = dirpath.join("dest");
                DefaultFileSystem.copy(&src_path, &dest).unwrap();
                let dest_content = read_to_string(&dest).unwrap();
                assert_eq!(src_file_content, dest_content);
            }
        }

        mod create_dir {
            use super::*;

            #[test]
            fn ok() {
                let path = tempdir().unwrap().into_path().join("test");
                DefaultFileSystem.create_dir(&path).unwrap();
                assert!(path.is_dir());
            }
        }

        mod create_temp_dir {
            use super::*;

            #[test]
            fn ok() {
                let path = DefaultFileSystem.create_temp_dir().unwrap();
                assert!(path.is_dir());
            }
        }

        mod cwd {
            use super::*;

            #[test]
            fn ok() {
                let cwd = DefaultFileSystem.cwd().unwrap();
                assert_eq!(cwd, current_dir().unwrap());
            }
        }

        mod delete_dir {
            use super::*;

            #[test]
            fn ok() {
                let path = tempdir().unwrap().into_path();
                DefaultFileSystem.delete_dir(&path).unwrap();
                assert!(!path.is_dir());
            }
        }

        mod open {
            use super::*;

            #[test]
            fn ok_when_w_mode_and_file_does_not_exist() {
                test(
                    |_| OpenOptions::new().create(true).write(true).to_owned(),
                    |mut file| write!(file, "test").unwrap(),
                );
            }

            #[test]
            fn ok_when_w_mode_and_file_exists() {
                test(
                    |path| {
                        File::create(path).unwrap();
                        OpenOptions::new().truncate(true).write(true).to_owned()
                    },
                    |mut file| write!(file, "test").unwrap(),
                );
            }

            #[inline]
            fn test<D: Fn(&Path) -> OpenOptions, A: Fn(File)>(data_from_fn: D, assert_fn: A) {
                let path = tempdir().unwrap().into_path().join("test");
                let opts = data_from_fn(&path);
                let file = DefaultFileSystem.open(&path, opts).unwrap();
                assert_fn(file);
            }
        }

        mod read_dir {
            use super::*;

            #[test]
            fn ok() {
                let path = tempdir().unwrap().into_path();
                let file1 = path.join("file1");
                File::create(&file1).unwrap();
                let file2 = path.join("file2");
                File::create(&file2).unwrap();
                let paths: HashSet<PathBuf> = DefaultFileSystem
                    .read_dir(&path)
                    .unwrap()
                    .map(|entry| entry.unwrap().path())
                    .collect();
                assert_eq!(paths.len(), 2);
                assert!(paths.contains(&file1));
                assert!(paths.contains(&file2));
            }
        }
    }
}
