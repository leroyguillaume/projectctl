use std::{collections::HashMap, fs::OpenOptions, path::Path};

use liquid::{
    model::{KString, ScalarCow, Value},
    Object, ParserBuilder,
};
use log::{debug, info, log_enabled, trace, Level};
#[cfg(test)]
use stub_trait::stub;

use crate::{
    err::{Error, LiquidErrorSource},
    fs::{DefaultFileSystem, FileSystem},
};

const GIT_DIRNAME: &str = ".git";
const LIQUID_EXTENSION: &str = "liquid";

pub type Result = std::result::Result<(), Error>;
pub type Vars = HashMap<String, String>;

#[cfg_attr(test, stub)]
pub trait Renderer {
    fn render_recursively(&self, tpl_dirpath: &Path, dest: &Path, vars: Vars) -> Result;
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
        let parser = ParserBuilder::with_stdlib().build().unwrap();
        debug!(
            "Rendering files from {} recursively into {}",
            tpl_dirpath.display(),
            dest.display()
        );
        for entry in self.fs.read_dir(tpl_dirpath)? {
            let entry = entry.map_err(Error::IO)?;
            let path = entry.path();
            let filename = path.file_name().unwrap().to_string_lossy();
            if filename == GIT_DIRNAME {
                trace!("Ignoring {} directory", GIT_DIRNAME);
            } else {
                debug!("Parsing filename `{}` as Liquid template", filename);
                let tpl = parser.parse(&filename).map_err(|cause| Error::Liquid {
                    cause,
                    src: LiquidErrorSource::Filename(filename.as_ref().into()),
                })?;
                trace!("Rendering filename");
                let dest_filename = tpl.render(&obj).map_err(|cause| Error::Liquid {
                    cause,
                    src: LiquidErrorSource::Filename(filename.as_ref().into()),
                })?;
                let dest = dest.join(dest_filename);
                if path.is_dir() {
                    self.fs.create_dir(&dest)?;
                    self.do_render_recursively(&path, &dest, obj)?;
                } else if path.is_file() {
                    if let Some(ext) = path.extension() {
                        if ext == LIQUID_EXTENSION {
                            debug!("Parsing {} as Liquid template", path.display());
                            let tpl = parser.parse_file(&path).map_err(|cause| Error::Liquid {
                                cause,
                                src: LiquidErrorSource::File(path.to_path_buf()),
                            })?;
                            let dest = dest.with_extension("");
                            let mut file = self.fs.open(
                                &dest,
                                OpenOptions::new()
                                    .create(true)
                                    .truncate(true)
                                    .write(true)
                                    .to_owned(),
                            )?;
                            debug!("Rendering into {}", dest.display());
                            tpl.render_to(&mut file, obj)
                                .map_err(|cause| Error::Liquid {
                                    cause,
                                    src: LiquidErrorSource::File(path.to_path_buf()),
                                })?;
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
    fn render_recursively(&self, tpl_dirpath: &Path, dest: &Path, vars: Vars) -> Result {
        info!(
            "Rendering files from template `{}`",
            tpl_dirpath.file_name().unwrap().to_string_lossy(),
        );
        if log_enabled!(Level::Debug) {
            let s = vars
                .iter()
                .map(|(key, val)| format!("{}: `{}`", key, val))
                .reduce(|accum, item| format!("{}, {}", accum, item))
                .unwrap_or_default();
            debug!("Variables: {{{}}}", s);
        }
        let mut obj = Object::new();
        for (key, val) in vars.into_iter() {
            obj.insert(KString::from(key), Value::Scalar(ScalarCow::from(val)));
        }
        self.do_render_recursively(tpl_dirpath, dest, &obj)
    }
}

#[cfg(test)]
mod test {
    use std::{
        fs::{create_dir_all, read_to_string, write},
        path::PathBuf,
    };

    use git2::Repository;
    use tempfile::tempdir;

    use super::*;

    mod default_renderer {
        use super::*;

        mod render_recursively {
            use super::*;

            struct Context<'a> {
                dest: &'a Path,
                src_dirname: &'a Path,
                static_file_content: &'a str,
                static_filename: &'a Path,
                tpl_dirpath: &'a Path,
                templated_filename: &'a Path,
                var_key: &'a str,
            }

            struct Parameters {
                project_dirname: PathBuf,
                templated_file_content: String,
                vars: Vars,
            }

            #[test]
            fn err_if_parse_filename_failed() {
                let project_dirname_fn =
                    |var_key: &str| PathBuf::from(format!("{{{{{} | min}}}}", var_key));
                test(
                    |ctx| Parameters {
                        project_dirname: project_dirname_fn(ctx.var_key),
                        templated_file_content: ctx.static_file_content.into(),
                        vars: Vars::from_iter(vec![(ctx.var_key.into(), "test".into())]),
                    },
                    |ctx, res| match res.unwrap_err() {
                        Error::Liquid { src, .. } => match src {
                            LiquidErrorSource::Filename(filename) => {
                                assert_eq!(filename, project_dirname_fn(ctx.var_key))
                            }
                            src => panic!("expected Filename (actual: {:?})", src),
                        },
                        err => panic!("expected Liquid (actual: {:?})", err),
                    },
                );
            }

            #[test]
            fn err_if_render_filename_failed() {
                let project_dirname_fn =
                    |var_key: &str| PathBuf::from(format!("{{{{{}2}}}}", var_key));
                test(
                    |ctx| Parameters {
                        project_dirname: project_dirname_fn(ctx.var_key),
                        templated_file_content: ctx.static_file_content.into(),
                        vars: Vars::from_iter(vec![(ctx.var_key.into(), "test".into())]),
                    },
                    |ctx, res| match res.unwrap_err() {
                        Error::Liquid { src, .. } => match src {
                            LiquidErrorSource::Filename(filename) => {
                                assert_eq!(filename, project_dirname_fn(ctx.var_key))
                            }
                            src => panic!("expected Filename (actual: {:?})", src),
                        },
                        err => panic!("expected Liquid (actual: {:?})", err),
                    },
                );
            }

            #[test]
            fn err_if_parse_file_failed() {
                let project_dirname_fn =
                    |var_key: &str| PathBuf::from(format!("{{{{{}}}}}", var_key));
                test(
                    |ctx| Parameters {
                        project_dirname: project_dirname_fn(ctx.var_key),
                        templated_file_content: "{{ | min }}".into(),
                        vars: Vars::from_iter(vec![(ctx.var_key.into(), "test".into())]),
                    },
                    |ctx, res| match res.unwrap_err() {
                        Error::Liquid { src, .. } => match src {
                            LiquidErrorSource::File(path) => {
                                let expected_path = ctx
                                    .tpl_dirpath
                                    .join(project_dirname_fn(ctx.var_key))
                                    .join(ctx.src_dirname)
                                    .join(ctx.templated_filename);
                                assert_eq!(path, expected_path);
                            }
                            src => panic!("expected File (actual: {:?})", src),
                        },
                        err => panic!("expected Liquid (actual: {:?})", err),
                    },
                );
            }

            #[test]
            fn err_if_render_file_failed() {
                let project_dirname_fn =
                    |var_key: &str| PathBuf::from(format!("{{{{{}}}}}", var_key));
                test(
                    |ctx| Parameters {
                        project_dirname: project_dirname_fn(ctx.var_key),
                        templated_file_content: format!("{{{{{}2}}}}", ctx.var_key),
                        vars: Vars::from_iter(vec![(ctx.var_key.into(), "test".into())]),
                    },
                    |ctx, res| match res.unwrap_err() {
                        Error::Liquid { src, .. } => match src {
                            LiquidErrorSource::File(path) => {
                                let expected_path = ctx
                                    .tpl_dirpath
                                    .join(project_dirname_fn(ctx.var_key))
                                    .join(ctx.src_dirname)
                                    .join(ctx.templated_filename);
                                assert_eq!(path, expected_path);
                            }
                            src => panic!("expected File (actual: {:?})", src),
                        },
                        err => panic!("expected Liquid (actual: {:?})", err),
                    },
                );
            }

            #[test]
            fn ok() {
                let var_val = "test";
                test(
                    |ctx| Parameters {
                        project_dirname: format!("{{{{{}}}}}", ctx.var_key).into(),
                        templated_file_content: ctx.static_file_content.into(),
                        vars: Vars::from_iter(vec![(ctx.var_key.into(), var_val.into())]),
                    },
                    |ctx, res| {
                        res.unwrap();
                        assert!(!ctx.dest.join(GIT_DIRNAME).exists());
                        let project_src_dirpath = ctx.dest.join(var_val).join(ctx.src_dirname);
                        let static_filepath = project_src_dirpath.join(ctx.static_filename);
                        let static_file_content = read_to_string(&static_filepath).unwrap();
                        assert_eq!(static_file_content, ctx.static_file_content);
                        let templated_filepath = project_src_dirpath.join(var_val);
                        let templated_file_content = read_to_string(&templated_filepath).unwrap();
                        assert_eq!(templated_file_content, var_val);
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
                let static_filename = Path::new("static");
                let var_key = "name";
                let static_file_content = format!("{{{{{}}}}}", var_key);
                let templated_filename =
                    PathBuf::from(format!("{{{{{}}}}}.{}", var_key, LIQUID_EXTENSION));
                let ctx = Context {
                    dest: &dest,
                    src_dirname: Path::new("src"),
                    static_file_content: &static_file_content,
                    static_filename,
                    tpl_dirpath: &tpl_dirpath,
                    templated_filename: &templated_filename,
                    var_key,
                };
                let params = data_from_fn(&ctx);
                let project_src_rel_dirpath = params.project_dirname.join(ctx.src_dirname);
                let project_src_dirpath = tpl_dirpath.join(&project_src_rel_dirpath);
                create_dir_all(&project_src_dirpath).unwrap();
                let static_rel_filepath = project_src_rel_dirpath.join("static");
                let static_filepath = tpl_dirpath.join(&static_rel_filepath);
                write(&static_filepath, &static_file_content).unwrap();
                let templated_rel_filepath = project_src_rel_dirpath.join(ctx.templated_filename);
                let templated_filepath = tpl_dirpath.join(&templated_rel_filepath);
                write(&templated_filepath, params.templated_file_content).unwrap();
                let renderer = LiquidRenderer {
                    fs: Box::new(DefaultFileSystem),
                };
                assert_fn(
                    &ctx,
                    renderer.render_recursively(&tpl_dirpath, &dest, params.vars),
                );
            }
        }
    }
}
