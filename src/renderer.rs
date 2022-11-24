use std::path::Path;

#[cfg(test)]
use stub_trait::stub;

use crate::{
    err::Error,
    fs::{DefaultFileSystem, FileSystem},
};

pub type Result = std::result::Result<(), Error>;

#[cfg_attr(test, stub)]
pub trait Renderer {
    fn render_recursively(&self, tpl_dirpath: &Path, dest: &Path) -> Result;
}

pub struct DefaultRenderer {
    fs: Box<dyn FileSystem>,
}

impl DefaultRenderer {
    pub fn new() -> Self {
        Self {
            fs: Box::new(DefaultFileSystem),
        }
    }
}

impl Renderer for DefaultRenderer {
    fn render_recursively(&self, tpl_dirpath: &Path, dest: &Path) -> Result {
        for entry in self.fs.read_dir(tpl_dirpath)? {
            let entry = entry.map_err(Error::IO)?;
            let path = entry.path();
            let filename = path.file_name().unwrap();
            let dest = dest.join(filename);
            if path.is_dir() {
                self.fs.create_dir(&dest)?;
                self.render_recursively(&path, &dest)?;
            } else if path.is_file() {
                self.fs.copy(&path, &dest)?;
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod test {
    use std::{
        fs::{copy, create_dir_all, read_dir, read_to_string, File},
        io::{self, Write},
    };

    use tempfile::tempdir;

    use crate::fs::{DirEntries, StubFileSystem};

    use super::*;

    mod default_renderer {
        use super::*;

        mod render_recursively {
            use super::*;

            type CopyFn = dyn Fn() -> io::Result<()>;
            type CreateDirFn = dyn Fn() -> io::Result<()>;
            type ReadDirFn = dyn Fn(&Path) -> io::Result<Box<DirEntries>>;

            struct Context<'a> {
                dest: &'a Path,
                static_file_content: &'a str,
                static_rel_filepath: &'a Path,
                templated_file_content: &'a str,
                templated_rel_filepath: &'a Path,
            }

            struct Parameters {
                copy_fn: Box<CopyFn>,
                create_dir_fn: Box<CreateDirFn>,
                read_dir_fn: Box<ReadDirFn>,
            }

            #[test]
            fn err_if_read_dir_failed() {
                let err_kind = io::ErrorKind::PermissionDenied;
                test(
                    |_| Parameters {
                        copy_fn: Box::new(|| Ok(())),
                        create_dir_fn: Box::new(|| Ok(())),
                        read_dir_fn: Box::new(move |_| Err(io::Error::from(err_kind))),
                    },
                    |_, res| match res.unwrap_err() {
                        Error::IO(_) => (),
                        err => {
                            let expected_err = io::Error::from(err_kind);
                            panic!("expected {:?} (actual: {:?})", expected_err, err);
                        }
                    },
                );
            }

            #[test]
            fn err_if_dir_entry_failed() {
                let err_kind = io::ErrorKind::PermissionDenied;
                test(
                    |_| Parameters {
                        copy_fn: Box::new(|| Ok(())),
                        create_dir_fn: Box::new(|| Ok(())),
                        read_dir_fn: Box::new(move |_| {
                            Ok(Box::new([Err(io::Error::from(err_kind))].into_iter()))
                        }),
                    },
                    |_, res| match res.unwrap_err() {
                        Error::IO(_) => (),
                        err => {
                            let expected_err = io::Error::from(err_kind);
                            panic!("expected {:?} (actual: {:?})", expected_err, err);
                        }
                    },
                );
            }

            #[test]
            fn err_if_create_dir_failed() {
                let err_kind = io::ErrorKind::PermissionDenied;
                test(
                    |_| Parameters {
                        copy_fn: Box::new(|| Ok(())),
                        create_dir_fn: Box::new(move || Err(io::Error::from(err_kind))),
                        read_dir_fn: Box::new(|path| {
                            read_dir(path).map(|read_dir| Box::new(read_dir) as Box<DirEntries>)
                        }),
                    },
                    |_, res| match res.unwrap_err() {
                        Error::IO(_) => (),
                        err => {
                            let expected_err = io::Error::from(err_kind);
                            panic!("expected {:?} (actual: {:?})", expected_err, err);
                        }
                    },
                );
            }

            #[test]
            fn err_if_copy_failed() {
                let err_kind = io::ErrorKind::PermissionDenied;
                test(
                    |_| Parameters {
                        copy_fn: Box::new(move || Err(io::Error::from(err_kind))),
                        create_dir_fn: Box::new(|| Ok(())),
                        read_dir_fn: Box::new(|path| {
                            read_dir(path).map(|read_dir| Box::new(read_dir) as Box<DirEntries>)
                        }),
                    },
                    |_, res| match res.unwrap_err() {
                        Error::IO(_) => (),
                        err => {
                            let expected_err = io::Error::from(err_kind);
                            panic!("expected {:?} (actual: {:?})", expected_err, err);
                        }
                    },
                );
            }

            #[test]
            fn ok() {
                test(
                    |_| Parameters {
                        copy_fn: Box::new(|| Ok(())),
                        create_dir_fn: Box::new(|| Ok(())),
                        read_dir_fn: Box::new(|path| {
                            read_dir(path).map(|read_dir| Box::new(read_dir) as Box<DirEntries>)
                        }),
                    },
                    |ctx, res| {
                        res.unwrap();
                        let static_filepath = ctx.dest.join(ctx.static_rel_filepath);
                        let static_file_content = read_to_string(&static_filepath).unwrap();
                        assert_eq!(static_file_content, ctx.static_file_content);
                        let templated_filepath = ctx.dest.join(ctx.templated_rel_filepath);
                        let templated_file_content = read_to_string(&templated_filepath).unwrap();
                        assert_eq!(templated_file_content, ctx.templated_file_content);
                    },
                );
            }

            #[inline]
            fn test<D: Fn(&Context) -> Parameters, A: Fn(&Context, Result)>(
                data_from_fn: D,
                assert_fn: A,
            ) {
                let dest = tempdir().unwrap().into_path();
                let tpl_dirpath = tempdir().unwrap().into_path();
                let project_src_rel_dirpath = Path::new("{{name}}/src");
                let project_src_dirpath = tpl_dirpath.join(project_src_rel_dirpath);
                create_dir_all(&project_src_dirpath).unwrap();
                let static_rel_filepath = project_src_rel_dirpath.join("static");
                let static_filepath = tpl_dirpath.join(&static_rel_filepath);
                let mut static_file = File::create(&static_filepath).unwrap();
                let static_file_content = "{{name}}";
                write!(static_file, "{}", static_file_content).unwrap();
                drop(static_file);
                let templated_rel_filepath = project_src_rel_dirpath.join("{{name}}.liquid");
                let templated_filepath = tpl_dirpath.join(&templated_rel_filepath);
                let mut templated_file = File::create(&templated_filepath).unwrap();
                write!(templated_file, "{}", static_file_content).unwrap();
                drop(templated_file);
                let ctx = Context {
                    dest: &dest,
                    static_file_content,
                    static_rel_filepath: &static_rel_filepath,
                    templated_file_content: static_file_content,
                    templated_rel_filepath: &templated_rel_filepath,
                };
                let params = data_from_fn(&ctx);
                let fs = StubFileSystem::new()
                    .with_stub_of_copy(move |_, src, dest| {
                        (params.copy_fn)()
                            .and_then(|_| copy(src, dest).map(|_| ()))
                            .map_err(Error::IO)
                    })
                    .with_stub_of_create_dir(move |_, path| {
                        (params.create_dir_fn)()
                            .and_then(|_| create_dir_all(path))
                            .map_err(Error::IO)
                    })
                    .with_stub_of_read_dir(move |_, path| {
                        (params.read_dir_fn)(path).map_err(Error::IO)
                    });
                let renderer = DefaultRenderer { fs: Box::new(fs) };
                assert_fn(&ctx, renderer.render_recursively(&tpl_dirpath, &dest));
            }
        }
    }
}
