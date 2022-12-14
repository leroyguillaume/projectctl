use std::{
    env::current_dir,
    fs::{create_dir_all, read_to_string, remove_dir_all, File, OpenOptions, ReadDir},
    io::{copy, Write},
    path::{Path, PathBuf},
};

use fs2::FileExt;
use home::home_dir;
use log::{debug, trace};
use tempfile::tempdir;

use crate::err::{Error, ErrorKind, Result};

macro_rules! write_into {
    ($path:expr, $file:expr, $($arg:tt)*) => {
        std::write!($file, $($arg)*).map_err(|err| {
            crate::err::Error {
                kind: crate::err::ErrorKind::IO(err),
                msg: format!("Unable to write into file {}", $path.display()),
            }
        })
    };
}
pub(crate) use write_into;

macro_rules! writeln_into {
    ($path:expr, $file:expr, $($arg:tt)*) => {
        std::writeln!($file, $($arg)*).map_err(|err| {
            crate::err::Error {
                kind: crate::err::ErrorKind::IO(err),
                msg: format!("Unable to write into file {}", $path.display()),
            }
        })
    };
}
pub(crate) use writeln_into;

#[cfg_attr(test, stub_trait::stub)]
pub trait FileSystem {
    fn canonicalize(&self, path: &Path) -> Result<PathBuf>;

    fn copy(&self, src: &Path, dest: &Path, lock: bool) -> Result<()>;

    fn create_dir(&self, path: &Path) -> Result<()>;

    fn create_temp_dir(&self) -> Result<PathBuf>;

    fn cwd(&self) -> Result<PathBuf>;

    fn delete_dir(&self, path: &Path) -> Result<()>;

    fn ensure_lines_are_absent(&self, lines: &[&str], path: &Path, lock: bool) -> Result<()>;

    fn ensure_lines_are_present(&self, lines: &[&str], path: &Path, lock: bool) -> Result<()>;

    fn home_dirpath(&self) -> Result<PathBuf>;

    fn open(&self, path: &Path, opts: OpenOptions, lock: bool) -> Result<File>;

    fn read_dir(&self, path: &Path) -> Result<ReadDir>;

    fn read_to_string(&self, path: &Path) -> Result<String>;
}

pub struct DefaultFileSystem;

impl FileSystem for DefaultFileSystem {
    fn canonicalize(&self, path: &Path) -> Result<PathBuf> {
        trace!("Canonicalizing {}", path.display());
        path.canonicalize().map_err(|err| Error {
            kind: ErrorKind::IO(err),
            msg: format!("Unable to canonicalize {}", path.display()),
        })
    }

    fn copy(&self, src: &Path, dest: &Path, lock: bool) -> Result<()> {
        if let Some(parent) = dest.parent() {
            self.create_dir(parent)?;
        }
        debug!("Copying {} into {}", src.display(), dest.display());
        let mut src_file = self.open(src, OpenOptions::new().read(true).to_owned(), false)?;
        let mut dest_file = self.open(
            dest,
            OpenOptions::new().create(true).write(true).to_owned(),
            lock,
        )?;
        copy(&mut src_file, &mut dest_file)
            .map(|len| trace!("{} bytes copied", len))
            .map_err(|err| Error {
                kind: ErrorKind::IO(err),
                msg: format!("Unable to copy {} into {}", src.display(), dest.display()),
            })
    }

    fn create_dir(&self, path: &Path) -> Result<()> {
        if !path.exists() {
            debug!("Creating directory {}", path.display());
            create_dir_all(path).map_err(|err| Error {
                kind: ErrorKind::IO(err),
                msg: format!("Unable to create directory {}", path.display()),
            })?;
        }
        Ok(())
    }

    fn create_temp_dir(&self) -> Result<PathBuf> {
        trace!("Creating temporary directory");
        tempdir()
            .map(|temp_dir| temp_dir.into_path())
            .map_err(|err| Error {
                kind: ErrorKind::IO(err),
                msg: "Unable to create temporary directory".into(),
            })
    }

    fn cwd(&self) -> Result<PathBuf> {
        trace!("Getting current working directory");
        current_dir().map_err(|err| Error {
            kind: ErrorKind::IO(err),
            msg: "Unable to get current working directory".into(),
        })
    }

    fn delete_dir(&self, path: &Path) -> Result<()> {
        debug!("Deleting directory {}", path.display());
        remove_dir_all(path).map_err(|err| Error {
            kind: ErrorKind::IO(err),
            msg: format!("Unable to to delete directory {}", path.display()),
        })
    }

    fn ensure_lines_are_absent(&self, lines: &[&str], path: &Path, lock: bool) -> Result<()> {
        if path.exists() {
            let content = self.read_to_string(path)?;
            let mut file = self.open(
                path,
                OpenOptions::new().truncate(true).write(true).to_owned(),
                lock,
            )?;
            for line in content.lines() {
                if !lines.contains(&line) {
                    writeln_into!(path, &mut file, "{}", line)?;
                }
            }
        } else {
            if let Some(parent) = path.parent() {
                self.create_dir(parent)?;
            }
            debug!("Creating file {}", path.display());
            File::create(path).map_err(|err| Error {
                kind: ErrorKind::IO(err),
                msg: format!("Unable to create file {}", path.display()),
            })?;
        }
        Ok(())
    }

    fn ensure_lines_are_present(&self, lines: &[&str], path: &Path, lock: bool) -> Result<()> {
        let content = if path.exists() {
            self.read_to_string(path)?
        } else {
            String::new()
        };
        let mut content_lines: Vec<&str> = content.lines().collect();
        let content_lines_len = content_lines.len();
        for line in lines {
            if !content_lines.contains(line) {
                content_lines.push(line);
            }
        }
        if content_lines_len != content_lines.len() {
            if let Some(parent) = path.parent() {
                self.create_dir(parent)?;
            }
            let mut file = self.open(
                path,
                OpenOptions::new()
                    .create(true)
                    .truncate(true)
                    .write(true)
                    .to_owned(),
                lock,
            )?;
            for line in content_lines {
                writeln_into!(path, &mut file, "{}", line)?;
            }
        }
        Ok(())
    }

    fn home_dirpath(&self) -> Result<PathBuf> {
        trace!("Getting home directory");
        home_dir().ok_or_else(|| Error {
            kind: ErrorKind::HomeNotFound,
            msg: "Unable to get home directory".into(),
        })
    }

    fn open(&self, path: &Path, opts: OpenOptions, lock: bool) -> Result<File> {
        trace!("Opening file {}", path.display());
        opts.open(path)
            .map_err(|err| Error {
                kind: ErrorKind::IO(err),
                msg: format!("Unable to open {}", path.display()),
            })
            .and_then(|file| {
                if lock {
                    trace!("Acquiring lock on {}", path.display());
                    file.lock_exclusive().map(|_| file).map_err(|err| Error {
                        kind: ErrorKind::IO(err),
                        msg: format!("Unable to acquire lock on {}", path.display()),
                    })
                } else {
                    Ok(file)
                }
            })
    }

    fn read_dir(&self, path: &Path) -> Result<ReadDir> {
        trace!("Reading directory {}", path.display());
        path.read_dir().map_err(|err| Error {
            kind: ErrorKind::IO(err),
            msg: format!("Unable to read directory {}", path.display()),
        })
    }

    fn read_to_string(&self, path: &Path) -> Result<String> {
        debug!("Reading file {}", path.display());
        read_to_string(path).map_err(|err| Error {
            kind: ErrorKind::IO(err),
            msg: format!("Unable to read file {}", path.display()),
        })
    }
}

#[cfg(test)]
mod test {
    use std::fs::write;

    use super::*;

    mod default_file_system {
        use super::*;

        mod canonicalize {
            use super::*;

            #[test]
            fn ok() {
                let path = Path::new("README.md");
                let expected_path = path.canonicalize().unwrap();
                let path = DefaultFileSystem.canonicalize(path).unwrap();
                assert_eq!(path, expected_path);
            }
        }

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
                    dest: root_dirpath.join("parent").join("dest"),
                    src: root_dirpath.join("src"),
                    src_content: "test",
                };
                let params = create_params_fn(&ctx);
                write(&ctx.src, ctx.src_content).unwrap();
                if let Some(content) = params.dest_content {
                    create_dir_all(ctx.dest.parent().unwrap()).unwrap();
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

        mod ensure_lines_are_absent {
            use super::*;

            struct Context {
                line1: &'static str,
                line2: &'static str,
                path: PathBuf,
            }

            struct Parameters {
                initial_content: Option<String>,
                lock: bool,
            }

            #[test]
            fn ok_when_file_does_not_exist() {
                test(
                    |_| Parameters {
                        initial_content: None,
                        lock: false,
                    },
                    |ctx, res| {
                        assert_file_content(ctx, res, "".into());
                    },
                )
            }

            #[test]
            fn ok_when_file_exists() {
                let initial_content = "line3";
                test(
                    |_| Parameters {
                        initial_content: Some(initial_content.into()),
                        lock: false,
                    },
                    |ctx, res| {
                        assert_file_content(ctx, res, format!("{}\n", initial_content));
                    },
                )
            }

            #[test]
            fn ok_when_file_contains_one_of_lines() {
                test(
                    |ctx| Parameters {
                        initial_content: Some(format!("\n{}\n\n", ctx.line1)),
                        lock: false,
                    },
                    |ctx, res| {
                        assert_file_content(ctx, res, "\n\n".into());
                    },
                )
            }

            #[test]
            fn ok_when_file_contains_all_lines() {
                test(
                    |ctx| Parameters {
                        initial_content: Some(format!("\n{}\n\n{}\n\n", ctx.line1, ctx.line2)),
                        lock: false,
                    },
                    |ctx, res| {
                        assert_file_content(ctx, res, "\n\n\n".into());
                    },
                )
            }

            fn assert_file_content(ctx: &Context, res: Result<()>, expected_content: String) {
                res.unwrap();
                let content = read_to_string(&ctx.path).unwrap();
                assert_eq!(content, expected_content);
            }

            fn test<P: Fn(&Context) -> Parameters, A: Fn(&Context, Result<()>)>(
                create_params_fn: P,
                assert_fn: A,
            ) {
                let ctx = Context {
                    line1: "line1",
                    line2: "line2",
                    path: tempdir().unwrap().into_path().join("parent").join("test"),
                };
                let params = create_params_fn(&ctx);
                if let Some(content) = params.initial_content {
                    create_dir_all(ctx.path.parent().unwrap()).unwrap();
                    write(&ctx.path, content).unwrap();
                }
                let res = DefaultFileSystem.ensure_lines_are_absent(
                    &[ctx.line1, ctx.line2],
                    &ctx.path,
                    params.lock,
                );
                assert_fn(&ctx, res);
            }
        }

        mod ensure_lines_are_present {
            use super::*;

            struct Context {
                line1: &'static str,
                line2: &'static str,
                path: PathBuf,
            }

            struct Parameters {
                initial_content: Option<String>,
                lock: bool,
            }

            #[test]
            fn ok_when_file_does_not_exist() {
                test(
                    |_| Parameters {
                        initial_content: None,
                        lock: false,
                    },
                    |ctx, res| {
                        assert_file_content(ctx, res, format!("{}\n{}\n", ctx.line1, ctx.line2));
                    },
                )
            }

            #[test]
            fn ok_when_file_exists() {
                let initial_content = "line3";
                test(
                    |_| Parameters {
                        initial_content: Some(initial_content.into()),
                        lock: false,
                    },
                    |ctx, res| {
                        assert_file_content(
                            ctx,
                            res,
                            format!("{}\n{}\n{}\n", initial_content, ctx.line1, ctx.line2),
                        );
                    },
                )
            }

            #[test]
            fn ok_when_file_contains_already_one_of_lines() {
                let initial_content_fn =
                    |ctx: &Context| -> String { format!("\n{}\n\n", ctx.line1) };
                test(
                    |ctx| Parameters {
                        initial_content: Some(initial_content_fn(ctx)),
                        lock: false,
                    },
                    |ctx, res| {
                        assert_file_content(
                            ctx,
                            res,
                            format!("{}{}\n", initial_content_fn(ctx), ctx.line2),
                        );
                    },
                )
            }

            #[test]
            fn ok_when_file_contains_already_all_lines() {
                let initial_content_fn =
                    |ctx: &Context| -> String { format!("\n{}\n\n{}\n\n", ctx.line1, ctx.line2) };
                test(
                    |ctx| Parameters {
                        initial_content: Some(initial_content_fn(ctx)),
                        lock: false,
                    },
                    |ctx, res| {
                        assert_file_content(ctx, res, initial_content_fn(ctx));
                    },
                )
            }

            fn assert_file_content(ctx: &Context, res: Result<()>, expected_content: String) {
                res.unwrap();
                let content = read_to_string(&ctx.path).unwrap();
                assert_eq!(content, expected_content);
            }

            fn test<P: Fn(&Context) -> Parameters, A: Fn(&Context, Result<()>)>(
                create_params_fn: P,
                assert_fn: A,
            ) {
                let ctx = Context {
                    line1: "line1",
                    line2: "line2",
                    path: tempdir().unwrap().into_path().join("parent").join("test"),
                };
                let params = create_params_fn(&ctx);
                if let Some(content) = params.initial_content {
                    create_dir_all(ctx.path.parent().unwrap()).unwrap();
                    write(&ctx.path, content).unwrap();
                }
                let res = DefaultFileSystem.ensure_lines_are_present(
                    &[ctx.line1, ctx.line2],
                    &ctx.path,
                    params.lock,
                );
                assert_fn(&ctx, res);
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
