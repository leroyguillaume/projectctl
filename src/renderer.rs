use std::{collections::HashMap, fs::OpenOptions, path::Path};

use liquid::{
    model::{KString, ScalarCow, Value},
    Object, ParserBuilder,
};
use log::{debug, info, log_enabled, trace, Level};
#[cfg(test)]
use stub_trait::stub;

use crate::{
    err::{Error, LiquidErrorSource, Result},
    fs::{DefaultFileSystem, FileSystem},
};

const GIT_DIRNAME: &str = ".git";
const LIQUID_EXTENSION: &str = "liquid";

pub type Vars = HashMap<String, String>;

#[cfg_attr(test, stub)]
pub trait Renderer {
    fn render_recursively(&self, tpl_dirpath: &Path, dest: &Path, vars: Vars) -> Result<()>;
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

    fn do_render_recursively(&self, tpl_dirpath: &Path, dest: &Path, obj: &Object) -> Result<()> {
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
                            trace!("Parsing {} as Liquid template", path.display());
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
                                false,
                            )?;
                            debug!("Rendering {} into {}", path.display(), dest.display());
                            tpl.render_to(&mut file, obj)
                                .map_err(|cause| Error::Liquid {
                                    cause,
                                    src: LiquidErrorSource::File(path.to_path_buf()),
                                })?;
                        } else {
                            self.fs.copy(&path, &dest, false)?;
                        }
                    } else {
                        self.fs.copy(&path, &dest, false)?;
                    }
                }
            }
        }
        Ok(())
    }
}

impl Renderer for LiquidRenderer {
    fn render_recursively(&self, tpl_dirpath: &Path, dest: &Path, vars: Vars) -> Result<()> {
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

            struct Context {
                dest: PathBuf,
                static_filename: &'static Path,
                tpl_dirpath: PathBuf,
                tpled_filename: PathBuf,
            }

            struct Parameters {
                files_content: String,
                tpled_dirname: PathBuf,
                vars: HashMap<String, String>,
            }

            #[test]
            fn err_when_filename_parsing_failed() {
                let var_key = "VAR";
                let var_val = "VAL";
                let tpled_dirname = PathBuf::from(format!("{{{{{} | min}}}}", var_key));
                test(
                    |_| Parameters {
                        files_content: format!("{{{{{}}}}}", var_key),
                        tpled_dirname: tpled_dirname.clone(),
                        vars: HashMap::from_iter([(var_key.into(), var_val.into())]),
                    },
                    |_, res| match res.unwrap_err() {
                        Error::Liquid { src, .. } => match src {
                            LiquidErrorSource::Filename(filename) => {
                                assert_eq!(filename, tpled_dirname)
                            }
                            src => panic!("expected Filename (actual: {:?})", src),
                        },
                        err => panic!("expected Liquid (actual: {:?})", err),
                    },
                );
            }

            #[test]
            fn err_when_filename_rendering_failed() {
                let var_key = "VAR";
                let var_val = "VAL";
                let tpled_dirname = PathBuf::from(format!("{{{{{}2}}}}", var_key));
                test(
                    |_| Parameters {
                        files_content: format!("{{{{{}}}}}", var_key),
                        tpled_dirname: tpled_dirname.clone(),
                        vars: HashMap::from_iter([(var_key.into(), var_val.into())]),
                    },
                    |_, res| match res.unwrap_err() {
                        Error::Liquid { src, .. } => match src {
                            LiquidErrorSource::Filename(filename) => {
                                assert_eq!(filename, tpled_dirname)
                            }
                            src => panic!("expected Filename (actual: {:?})", src),
                        },
                        err => panic!("expected Liquid (actual: {:?})", err),
                    },
                );
            }

            #[test]
            fn err_when_file_parsing_failed() {
                let var_key = "VAR";
                let var_val = "VAL";
                let tpled_dirname = PathBuf::from(format!("{{{{{}}}}}", var_key));
                test(
                    {
                        let tpled_dirname = tpled_dirname.clone();
                        move |_| Parameters {
                            files_content: format!("{{{{{} | min}}}}", var_key),
                            tpled_dirname: tpled_dirname.clone(),
                            vars: HashMap::from_iter([(var_key.into(), var_val.into())]),
                        }
                    },
                    |ctx, res| match res.unwrap_err() {
                        Error::Liquid { src, .. } => match src {
                            LiquidErrorSource::File(path) => {
                                let expected_path = ctx
                                    .tpl_dirpath
                                    .join(&tpled_dirname)
                                    .join(&ctx.tpled_filename);
                                assert_eq!(path, expected_path);
                            }
                            src => panic!("expected Filename (actual: {:?})", src),
                        },
                        err => panic!("expected Liquid (actual: {:?})", err),
                    },
                );
            }

            #[test]
            fn err_when_file_rendering_failed() {
                let var_key = "VAR";
                let var_val = "VAL";
                let tpled_dirname = PathBuf::from(format!("{{{{{}}}}}", var_key));
                test(
                    {
                        let tpled_dirname = tpled_dirname.clone();
                        move |_| Parameters {
                            files_content: format!("{{{{{}2}}}}", var_key),
                            tpled_dirname: tpled_dirname.clone(),
                            vars: HashMap::from_iter([(var_key.into(), var_val.into())]),
                        }
                    },
                    |ctx, res| match res.unwrap_err() {
                        Error::Liquid { src, .. } => match src {
                            LiquidErrorSource::File(path) => {
                                let expected_path = ctx
                                    .tpl_dirpath
                                    .join(&tpled_dirname)
                                    .join(&ctx.tpled_filename);
                                assert_eq!(path, expected_path);
                            }
                            src => panic!("expected Filename (actual: {:?})", src),
                        },
                        err => panic!("expected Liquid (actual: {:?})", err),
                    },
                );
            }

            #[test]
            fn ok() {
                let var_key = "VAR";
                let var_val = "VAL";
                let files_content = format!("{{{{{}}}}}", var_key);
                test(
                    |_| Parameters {
                        files_content: files_content.clone(),
                        tpled_dirname: PathBuf::from(format!("{{{{{}}}}}", var_key)),
                        vars: HashMap::from_iter([(var_key.into(), var_val.into())]),
                    },
                    |ctx, res| {
                        res.unwrap();
                        let dirpath = ctx.dest.join(var_val);
                        let static_filepath = dirpath.join(ctx.static_filename);
                        let static_file_content = read_to_string(static_filepath).unwrap();
                        assert_eq!(static_file_content, files_content);
                        let tpled_filepath = dirpath.join(ctx.tpled_filename.with_extension(""));
                        let tpled_file_content = read_to_string(tpled_filepath).unwrap();
                        assert_eq!(tpled_file_content, var_val);
                        assert!(!ctx.dest.join(GIT_DIRNAME).exists());
                    },
                );
            }

            fn test<P: Fn(&Context) -> Parameters, A: Fn(&Context, Result<()>)>(
                create_params_fn: P,
                assert_fn: A,
            ) {
                let ctx = Context {
                    dest: tempdir().unwrap().into_path(),
                    static_filename: Path::new("static"),
                    tpl_dirpath: tempdir().unwrap().into_path(),
                    tpled_filename: format!("templated.{}", LIQUID_EXTENSION).into(),
                };
                let params = create_params_fn(&ctx);
                let tpled_dirpath = ctx.tpl_dirpath.join(params.tpled_dirname);
                let static_filepath = tpled_dirpath.join(ctx.static_filename);
                let tpled_filepath = tpled_dirpath.join(&ctx.tpled_filename);
                create_dir_all(&tpled_dirpath).unwrap();
                write(static_filepath, &params.files_content).unwrap();
                write(tpled_filepath, &params.files_content).unwrap();
                Repository::init(&ctx.tpl_dirpath).unwrap();
                let renderer = LiquidRenderer {
                    fs: Box::new(DefaultFileSystem),
                };
                let res = renderer.render_recursively(&ctx.tpl_dirpath, &ctx.dest, params.vars);
                assert_fn(&ctx, res);
            }
        }
    }
}
