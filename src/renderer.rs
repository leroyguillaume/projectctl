use std::{fs::OpenOptions, path::Path};

use liquid::{
    model::{KString, ScalarCow, Value},
    Object, ParserBuilder,
};
use log::debug;
#[cfg(test)]
use stub_trait::stub;

use crate::{
    err::Error,
    fs::{DefaultFileSystem, FileSystem},
};

const GIT_DIRNAME: &str = ".git";
const LIQUID_EXTENSION: &str = "liquid";

pub type Result = std::result::Result<(), Error>;

#[cfg_attr(test, stub)]
pub trait Renderer {
    fn render_recursively(
        &self,
        tpl_dirpath: &Path,
        dest: &Path,
        vars: Vec<(String, String)>,
    ) -> Result;
}

pub struct LiquidRenderer {
    fs: Box<dyn FileSystem>,
}

impl LiquidRenderer {
    pub fn new() -> Self {
        Self {
            fs: Box::new(DefaultFileSystem),
        }
    }

    fn do_render_recursively(&self, tpl_dirpath: &Path, dest: &Path, obj: &Object) -> Result {
        debug!(
            "Rendering files from {} recursively into {}",
            tpl_dirpath.display(),
            dest.display()
        );
        for entry in self.fs.read_dir(tpl_dirpath)? {
            let entry = entry.map_err(Error::IO)?;
            let path = entry.path();
            let filename = path.file_name().unwrap();
            if filename == GIT_DIRNAME {
                debug!("Ignoring {} directory", GIT_DIRNAME);
            } else {
                let dest = dest.join(filename);
                if path.is_dir() {
                    self.fs.create_dir(&dest)?;
                    self.do_render_recursively(&path, &dest, obj)?;
                } else if path.is_file() {
                    if let Some(ext) = path.extension() {
                        if ext == LIQUID_EXTENSION {
                            debug!("Parsing {} as Liquid template", path.display());
                            let tpl = ParserBuilder::with_stdlib()
                                .build()
                                .unwrap()
                                .parse_file(&path)
                                .map_err(Error::Liquid)?;
                            let mut file = self.fs.open(
                                &dest,
                                OpenOptions::new()
                                    .create(true)
                                    .truncate(true)
                                    .write(true)
                                    .to_owned(),
                            )?;
                            debug!("Rendering into {}", dest.display());
                            tpl.render_to(&mut file, obj).map_err(Error::Liquid)?;
                        } else {
                            self.fs.copy(&path, &dest)?;
                        }
                    } else {
                        self.fs.copy(&path, &dest)?;
                    }
                }
            }
        }
        Ok(())
    }
}

impl Renderer for LiquidRenderer {
    fn render_recursively(
        &self,
        tpl_dirpath: &Path,
        dest: &Path,
        vars: Vec<(String, String)>,
    ) -> Result {
        let mut obj = Object::new();
        for (key, value) in vars.into_iter() {
            obj.insert(KString::from(key), Value::Scalar(ScalarCow::from(value)));
        }
        self.do_render_recursively(tpl_dirpath, dest, &obj)
    }
}

#[cfg(test)]
mod test {
    use std::{
        fs::{copy, create_dir_all, read_dir, read_to_string, File},
        io::{self, Write},
        path::PathBuf,
    };

    use git2::Repository;
    use tempfile::tempdir;

    use crate::fs::{DirEntries, StubFileSystem};

    use super::*;

    mod default_renderer {
        use super::*;

        mod render_recursively {
            use super::*;

            type CopyFn = dyn Fn() -> io::Result<()>;
            type CreateDirFn = dyn Fn() -> io::Result<()>;
            type OpenFn = dyn Fn() -> io::Result<()>;
            type ReadDirFn = dyn Fn(&Path) -> io::Result<Box<DirEntries>>;

            struct Context<'a> {
                dest: &'a Path,
                static_file_content: &'a str,
                static_rel_filepath: &'a Path,
                templated_rel_filepath: &'a Path,
                var_name: &'a str,
            }

            struct Parameters {
                copy_fn: Box<CopyFn>,
                create_dir_fn: Box<CreateDirFn>,
                open_fn: Box<OpenFn>,
                read_dir_fn: Box<ReadDirFn>,
                templated_file_content: String,
                vars: Vec<(String, String)>,
            }

            #[test]
            fn err_if_read_dir_failed() {
                test(
                    |ctx| Parameters {
                        copy_fn: Box::new(|| Ok(())),
                        create_dir_fn: Box::new(|| Ok(())),
                        open_fn: Box::new(|| Ok(())),
                        read_dir_fn: Box::new(move |_| {
                            Err(io::Error::from(io::ErrorKind::PermissionDenied))
                        }),
                        templated_file_content: ctx.static_file_content.into(),
                        vars: vec![(ctx.var_name.into(), "test".into())],
                    },
                    |_, res| match res.unwrap_err() {
                        Error::IO(_) => (),
                        err => panic!("expected IO (actual: {:?})", err),
                    },
                );
            }

            #[test]
            fn err_if_dir_entry_failed() {
                test(
                    |ctx| Parameters {
                        copy_fn: Box::new(|| Ok(())),
                        create_dir_fn: Box::new(|| Ok(())),
                        open_fn: Box::new(|| Ok(())),
                        read_dir_fn: Box::new(move |_| {
                            Ok(Box::new(
                                [Err(io::Error::from(io::ErrorKind::PermissionDenied))].into_iter(),
                            ))
                        }),
                        templated_file_content: ctx.static_file_content.into(),
                        vars: vec![(ctx.var_name.into(), "test".into())],
                    },
                    |_, res| match res.unwrap_err() {
                        Error::IO(_) => (),
                        err => panic!("expected IO (actual: {:?})", err),
                    },
                );
            }

            #[test]
            fn err_if_create_dir_failed() {
                test(
                    |ctx| Parameters {
                        copy_fn: Box::new(|| Ok(())),
                        create_dir_fn: Box::new(move || {
                            Err(io::Error::from(io::ErrorKind::PermissionDenied))
                        }),
                        open_fn: Box::new(|| Ok(())),
                        read_dir_fn: Box::new(|path| {
                            read_dir(path).map(|read_dir| Box::new(read_dir) as Box<DirEntries>)
                        }),
                        templated_file_content: ctx.static_file_content.into(),
                        vars: vec![(ctx.var_name.into(), "test".into())],
                    },
                    |_, res| match res.unwrap_err() {
                        Error::IO(_) => (),
                        err => panic!("expected IO (actual: {:?})", err),
                    },
                );
            }

            #[test]
            fn err_if_parse_failed() {
                test(
                    |ctx| Parameters {
                        copy_fn: Box::new(|| Ok(())),
                        create_dir_fn: Box::new(|| Ok(())),
                        open_fn: Box::new(|| Ok(())),
                        read_dir_fn: Box::new(|path| {
                            read_dir(path).map(|read_dir| Box::new(read_dir) as Box<DirEntries>)
                        }),
                        templated_file_content: "{{ | min }}".into(),
                        vars: vec![(ctx.var_name.into(), "test".into())],
                    },
                    |_, res| match res.unwrap_err() {
                        Error::Liquid(_) => {}
                        err => panic!("expected Liquid (actual: {:?})", err),
                    },
                );
            }

            #[test]
            fn err_if_render_failed() {
                test(
                    |ctx| Parameters {
                        copy_fn: Box::new(|| Ok(())),
                        create_dir_fn: Box::new(|| Ok(())),
                        open_fn: Box::new(|| Ok(())),
                        read_dir_fn: Box::new(|path| {
                            read_dir(path).map(|read_dir| Box::new(read_dir) as Box<DirEntries>)
                        }),
                        templated_file_content: ctx.static_file_content.into(),
                        vars: vec![],
                    },
                    |_, res| match res.unwrap_err() {
                        Error::Liquid(_) => {}
                        err => panic!("expected Liquid (actual: {:?})", err),
                    },
                );
            }

            #[test]
            fn err_if_copy_failed() {
                test(
                    |ctx| Parameters {
                        copy_fn: Box::new(move || {
                            Err(io::Error::from(io::ErrorKind::PermissionDenied))
                        }),
                        create_dir_fn: Box::new(|| Ok(())),
                        open_fn: Box::new(|| Ok(())),
                        read_dir_fn: Box::new(|path| {
                            read_dir(path).map(|read_dir| Box::new(read_dir) as Box<DirEntries>)
                        }),
                        templated_file_content: ctx.static_file_content.into(),
                        vars: vec![(ctx.var_name.into(), "test".into())],
                    },
                    |_, res| match res.unwrap_err() {
                        Error::IO(_) => (),
                        err => panic!("expected IO (actual: {:?})", err),
                    },
                );
            }

            #[test]
            fn ok() {
                let var_value = "test";
                test(
                    |ctx| Parameters {
                        copy_fn: Box::new(|| Ok(())),
                        create_dir_fn: Box::new(|| Ok(())),
                        open_fn: Box::new(|| Ok(())),
                        read_dir_fn: Box::new(|path| {
                            read_dir(path).map(|read_dir| Box::new(read_dir) as Box<DirEntries>)
                        }),
                        templated_file_content: ctx.static_file_content.into(),
                        vars: vec![(ctx.var_name.into(), var_value.into())],
                    },
                    |ctx, res| {
                        res.unwrap();
                        assert!(!ctx.dest.join(GIT_DIRNAME).exists());
                        let static_filepath = ctx.dest.join(ctx.static_rel_filepath);
                        let static_file_content = read_to_string(&static_filepath).unwrap();
                        assert_eq!(static_file_content, ctx.static_file_content);
                        let templated_filepath = ctx.dest.join(ctx.templated_rel_filepath);
                        let templated_file_content = read_to_string(&templated_filepath).unwrap();
                        assert_eq!(templated_file_content, var_value);
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
                Repository::init(&tpl_dirpath).unwrap();
                let var_name = "name";
                let project_src_rel_dirpath = PathBuf::from(&format!("{{{{{}}}}}/src", var_name));
                let project_src_dirpath = tpl_dirpath.join(&project_src_rel_dirpath);
                create_dir_all(&project_src_dirpath).unwrap();
                let static_rel_filepath = project_src_rel_dirpath.join("static");
                let static_filepath = tpl_dirpath.join(&static_rel_filepath);
                let mut static_file = File::create(&static_filepath).unwrap();
                let static_file_content = format!("{{{{{}}}}}", var_name);
                write!(static_file, "{}", static_file_content).unwrap();
                drop(static_file);
                let templated_rel_filepath =
                    project_src_rel_dirpath.join(format!("{{{{{}}}}}.liquid", var_name));
                let templated_filepath = tpl_dirpath.join(&templated_rel_filepath);
                let ctx = Context {
                    dest: &dest,
                    static_file_content: &static_file_content,
                    static_rel_filepath: &static_rel_filepath,
                    templated_rel_filepath: &templated_rel_filepath,
                    var_name,
                };
                let params = data_from_fn(&ctx);
                let mut templated_file = File::create(&templated_filepath).unwrap();
                write!(templated_file, "{}", params.templated_file_content).unwrap();
                drop(templated_file);
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
                    .with_stub_of_open(move |_, path, opts| {
                        (params.open_fn)()
                            .and_then(|_| opts.open(path))
                            .map_err(Error::IO)
                    })
                    .with_stub_of_read_dir(move |_, path| {
                        (params.read_dir_fn)(path).map_err(Error::IO)
                    });
                let renderer = LiquidRenderer { fs: Box::new(fs) };
                assert_fn(
                    &ctx,
                    renderer.render_recursively(&tpl_dirpath, &dest, params.vars),
                );
            }
        }
    }
}
