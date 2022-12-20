use std::{
    env::current_dir,
    fs::{create_dir_all, read_to_string, remove_dir_all, File, OpenOptions, ReadDir},
    io::{self, copy},
    path::{Path, PathBuf},
};

use fs2::FileExt;
use home::home_dir;
use log::{debug, trace};
#[cfg(test)]
use stub_trait::stub;
use tempfile::tempdir;

use crate::err::{Error, Result};

#[cfg_attr(test, stub)]
pub trait FileSystem {
    fn copy(&self, src: &Path, dest: &Path, lock: bool) -> Result<()>;

    fn create_dir(&self, path: &Path) -> Result<()>;

    fn create_temp_dir(&self) -> Result<PathBuf>;

    fn cwd(&self) -> Result<PathBuf>;

    fn delete_dir(&self, path: &Path) -> Result<()>;

    fn home_dirpath(&self) -> Result<PathBuf>;

    fn open(&self, path: &Path, opts: OpenOptions, lock: bool) -> Result<File>;

    fn read_dir(&self, path: &Path) -> Result<ReadDir>;

    fn read_to_string(&self, path: &Path) -> Result<String>;
}

pub struct DefaultFileSystem;

impl FileSystem for DefaultFileSystem {
    fn copy(&self, src: &Path, dest: &Path, lock: bool) -> Result<()> {
        debug!("Copying {} into {}", src.display(), dest.display());
        let mut src_file = self.open(src, OpenOptions::new().read(true).to_owned(), false)?;
        let mut dest_file = self.open(
            dest,
            OpenOptions::new().create(true).write(true).to_owned(),
            lock,
        )?;
        copy(&mut src_file, &mut dest_file)
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

    fn open(&self, path: &Path, opts: OpenOptions, lock: bool) -> Result<File> {
        trace!("Opening file {}", path.display());
        opts.open(path)
            .map_err(|err| {
                Error::IO(io::Error::new(
                    err.kind(),
                    format!("Unable to open {}: {}", path.display(), err),
                ))
            })
            .and_then(|file| {
                if lock {
                    trace!("Acquiring lock on {}", path.display());
                    file.lock_exclusive().map(|_| file).map_err(|err| {
                        Error::IO(io::Error::new(
                            err.kind(),
                            format!("Unable to acquire lock on {}: {}", path.display(), err),
                        ))
                    })
                } else {
                    Ok(file)
                }
            })
    }

    fn read_dir(&self, path: &Path) -> Result<ReadDir> {
        trace!("Reading directory {}", path.display());
        path.read_dir().map_err(|err| {
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

#[cfg(test)]
mod test {
    use std::{fs::write, io::Write};

    use super::*;

    mod default_file_system {
        use super::*;

        mod copy {
            use super::*;

            struct Context {
                dest: PathBuf,
                src: PathBuf,
                src_content: &'static str,
            }

            struct Parameters {
                dest_content: Option<&'static str>,
                lock: bool,
            }

            #[test]
            fn ok_when_dest_does_not_exist() {
                test(
                    |_| Parameters {
                        dest_content: None,
                        lock: false,
                    },
                    assert,
                )
            }

            #[test]
            fn ok_when_dest_exists() {
                test(
                    |_| Parameters {
                        dest_content: Some("dest"),
                        lock: false,
                    },
                    assert,
                )
            }

            #[test]
            fn ok_when_lock_is_true() {
                test(
                    |_| Parameters {
                        dest_content: None,
                        lock: true,
                    },
                    assert,
                )
            }

            fn assert(ctx: &Context, res: Result<()>) {
                res.unwrap();
                let content = read_to_string(&ctx.dest).unwrap();
                assert_eq!(content, ctx.src_content);
            }

            fn test<P: Fn(&Context) -> Parameters, A: Fn(&Context, Result<()>)>(
                create_params_fn: P,
                assert_fn: A,
            ) {
                let root_dirpath = tempdir().unwrap().into_path();
                let ctx = Context {
                    dest: root_dirpath.join("dest"),
                    src: root_dirpath.join("src"),
                    src_content: "test",
                };
                let params = create_params_fn(&ctx);
                write(&ctx.src, ctx.src_content).unwrap();
                if let Some(content) = params.dest_content {
                    write(&ctx.dest, content).unwrap();
                }
                let res = DefaultFileSystem.copy(&ctx.src, &ctx.dest, params.lock);
                assert_fn(&ctx, res);
            }
        }

        mod create_dir {
            use super::*;

            #[test]
            fn ok() {
                let path = tempdir().unwrap().into_path().join("parent").join("child");
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
                let expected_path = current_dir().unwrap();
                let path = DefaultFileSystem.cwd().unwrap();
                assert_eq!(path, expected_path);
            }
        }

        mod delete_dir {
            use super::*;

            #[test]
            fn ok() {
                let root_dirpath = tempdir().unwrap().into_path();
                let path = root_dirpath.join("child1");
                create_dir_all(path).unwrap();
                DefaultFileSystem.delete_dir(&root_dirpath).unwrap();
                assert!(!root_dirpath.exists());
            }
        }

        mod home_dirpath {
            use super::*;

            #[test]
            fn ok() {
                let expected_path = home_dir().unwrap();
                let path = DefaultFileSystem.home_dirpath().unwrap();
                assert_eq!(path, expected_path);
            }
        }

        mod open {
            use super::*;

            struct Context {
                path: PathBuf,
            }

            struct Parameters {
                lock: bool,
            }

            #[test]
            fn ok_when_lock_is_false() {
                test(
                    |_| Parameters { lock: false },
                    |_, res| {
                        let mut file = res.unwrap();
                        write!(&mut file, "test").unwrap();
                    },
                );
            }

            #[test]
            fn ok_when_lock_is_true() {
                test(
                    |_| Parameters { lock: true },
                    |ctx, res| {
                        let mut file = res.unwrap();
                        write!(&mut file, "test").unwrap();
                        let file2 = File::open(&ctx.path).unwrap();
                        assert!(file2.try_lock_exclusive().is_err());
                    },
                );
            }

            fn test<P: Fn(&Context) -> Parameters, A: Fn(&Context, Result<File>)>(
                create_params_fn: P,
                assert_fn: A,
            ) {
                let ctx = Context {
                    path: tempdir().unwrap().into_path().join("test"),
                };
                let params = create_params_fn(&ctx);
                let res = DefaultFileSystem.open(
                    &ctx.path,
                    OpenOptions::new().create(true).write(true).to_owned(),
                    params.lock,
                );
                assert_fn(&ctx, res);
            }
        }

        mod read_dir {
            use super::*;

            #[test]
            fn ok() {
                let root_dirpath = tempdir().unwrap().into_path();
                let path1 = root_dirpath.join("test1");
                let path2 = root_dirpath.join("test2");
                File::create(&path1).unwrap();
                File::create(&path2).unwrap();
                let paths: Vec<PathBuf> = DefaultFileSystem
                    .read_dir(&root_dirpath)
                    .unwrap()
                    .map(|entry| entry.unwrap().path())
                    .collect();
                assert_eq!(paths.len(), 2);
                assert!(paths.contains(&path1));
                assert!(paths.contains(&path2));
            }
        }

        mod read_to_string {
            use super::*;

            #[test]
            fn ok() {
                let expected_str = "test";
                let path = tempdir().unwrap().into_path().join("test");
                write(&path, expected_str).unwrap();
                let str = DefaultFileSystem.read_to_string(&path).unwrap();
                assert_eq!(str, expected_str);
            }
        }
    }
}
