use std::path::PathBuf;

use crate::{
    err::Result,
    fs::{DefaultFileSystem, FileSystem},
};

const DEFAULT_CONFIG_DIRNAME: &str = ".projectctl";
const DEFAULT_ALLOWED_DIRS_FILENAME: &str = "allowed-dirs";

#[cfg_attr(test, stub_trait::stub)]
pub trait Paths {
    fn allowed_dirs(
        &self,
        allowed_dirs_filepath: Option<PathBuf>,
        cfg_dirpath: Option<PathBuf>,
    ) -> Result<PathBuf>;

    fn config(&self, cfg_dirpath: Option<PathBuf>) -> Result<PathBuf>;
}

pub struct DefaultPaths {
    fs: Box<dyn FileSystem>,
}

impl DefaultPaths {
    pub fn new() -> Self {
        Self {
            fs: Box::new(DefaultFileSystem),
        }
    }
}

impl Paths for DefaultPaths {
    fn allowed_dirs(
        &self,
        allowed_dirs_filepath: Option<PathBuf>,
        cfg_dirpath: Option<PathBuf>,
    ) -> Result<PathBuf> {
        self.config(cfg_dirpath).map(|path| {
            path.join(allowed_dirs_filepath.unwrap_or_else(|| DEFAULT_ALLOWED_DIRS_FILENAME.into()))
        })
    }

    fn config(&self, cfg_dirpath: Option<PathBuf>) -> Result<PathBuf> {
        cfg_dirpath.map(Ok).unwrap_or_else(|| {
            self.fs
                .home_dirpath()
                .map(|path| path.join(DEFAULT_CONFIG_DIRNAME))
        })
    }
}

#[cfg(test)]
mod test {
    use tempfile::tempdir;

    use crate::fs::StubFileSystem;

    use super::*;

    mod default_paths {
        use super::*;

        mod allowed_dirs {
            use super::*;

            struct Context {
                home_dirpath: PathBuf,
            }

            struct Parameters {
                allowed_dirs_filepath: Option<PathBuf>,
                cfg_dirpath: Option<PathBuf>,
            }

            #[test]
            fn ok_when_no_override() {
                test(
                    |_| Parameters {
                        allowed_dirs_filepath: None,
                        cfg_dirpath: None,
                    },
                    |ctx, res| {
                        assert(
                            res,
                            ctx.home_dirpath
                                .join(DEFAULT_CONFIG_DIRNAME)
                                .join(DEFAULT_ALLOWED_DIRS_FILENAME),
                        );
                    },
                )
            }

            #[test]
            fn ok_when_cfg_dirpath_is_overriden() {
                let cfg_dirpath = tempdir().unwrap().into_path();
                test(
                    |_| Parameters {
                        allowed_dirs_filepath: None,
                        cfg_dirpath: Some(cfg_dirpath.clone()),
                    },
                    |_, res| {
                        assert(res, cfg_dirpath.join(DEFAULT_ALLOWED_DIRS_FILENAME));
                    },
                )
            }

            #[test]
            fn ok_when_allowed_dirs_filepath_is_overriden() {
                let allowed_dirs_filepath = tempdir().unwrap().into_path().join("test");
                let cfg_dirpath = tempdir().unwrap().into_path();
                test(
                    {
                        let allowed_dirs_filepath = allowed_dirs_filepath.clone();
                        move |_| Parameters {
                            allowed_dirs_filepath: Some(allowed_dirs_filepath.clone()),
                            cfg_dirpath: Some(cfg_dirpath.clone()),
                        }
                    },
                    move |_, res| {
                        assert(res, allowed_dirs_filepath.clone());
                    },
                )
            }

            fn assert(res: Result<PathBuf>, expected_path: PathBuf) {
                let path = res.unwrap();
                assert_eq!(path, expected_path);
            }

            fn test<P: Fn(&Context) -> Parameters, A: Fn(&Context, Result<PathBuf>)>(
                create_params_fn: P,
                assert_fn: A,
            ) {
                let ctx = Context {
                    home_dirpath: tempdir().unwrap().into_path(),
                };
                let params = create_params_fn(&ctx);
                let fs = StubFileSystem::new().with_stub_of_home_dirpath({
                    let home_dirpath = ctx.home_dirpath.clone();
                    move |_| Ok(home_dirpath.clone())
                });
                let paths = DefaultPaths { fs: Box::new(fs) };
                let res = paths.allowed_dirs(params.allowed_dirs_filepath, params.cfg_dirpath);
                assert_fn(&ctx, res);
            }
        }

        mod config {
            use super::*;

            struct Context {
                home_dirpath: PathBuf,
            }

            struct Parameters {
                cfg_dirpath: Option<PathBuf>,
            }

            #[test]
            fn ok_when_no_override() {
                test(
                    |_| Parameters { cfg_dirpath: None },
                    |ctx, res| {
                        assert(res, ctx.home_dirpath.join(DEFAULT_CONFIG_DIRNAME));
                    },
                )
            }

            #[test]
            fn ok_when_cfg_dirpath_is_overriden() {
                let cfg_dirpath = tempdir().unwrap().into_path();
                test(
                    |_| Parameters {
                        cfg_dirpath: Some(cfg_dirpath.clone()),
                    },
                    |_, res| {
                        assert(res, cfg_dirpath.clone());
                    },
                )
            }

            fn assert(res: Result<PathBuf>, expected_path: PathBuf) {
                let path = res.unwrap();
                assert_eq!(path, expected_path);
            }

            fn test<P: Fn(&Context) -> Parameters, A: Fn(&Context, Result<PathBuf>)>(
                create_params_fn: P,
                assert_fn: A,
            ) {
                let ctx = Context {
                    home_dirpath: tempdir().unwrap().into_path(),
                };
                let params = create_params_fn(&ctx);
                let fs = StubFileSystem::new().with_stub_of_home_dirpath({
                    let home_dirpath = ctx.home_dirpath.clone();
                    move |_| Ok(home_dirpath.clone())
                });
                let paths = DefaultPaths { fs: Box::new(fs) };
                let res = paths.config(params.cfg_dirpath);
                assert_fn(&ctx, res);
            }
        }
    }
}
