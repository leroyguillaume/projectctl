use std::path::PathBuf;

use crate::{
    err::Result,
    fs::{DefaultFileSystem, FileSystem},
};

pub const LOCAL_CONFIG_FILENAME: &str = "projectctl.local.yml";
pub const PROJECT_CONFIG_FILENAME: &str = "projectctl.yml";

const DEFAULT_PROJECTCTL_DIRNAME: &str = ".projectctl";
const DEFAULT_ALLOWED_DIRS_FILENAME: &str = "allowed-dirs";

#[cfg_attr(test, stub_trait::stub)]
pub trait Paths {
    fn allowed_dirs(
        &self,
        allowed_dirs_filepath: Option<PathBuf>,
        projectctl_dirpath: Option<PathBuf>,
    ) -> Result<PathBuf>;

    fn config(
        &self,
        cfg_filepaths: Vec<PathBuf>,
        project_dirpath: Option<PathBuf>,
    ) -> Result<Vec<PathBuf>>;

    fn projectctl_dir(&self, projectctl_dirpath: Option<PathBuf>) -> Result<PathBuf>;
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
        projectctl_dirpath: Option<PathBuf>,
    ) -> Result<PathBuf> {
        self.projectctl_dir(projectctl_dirpath).map(|path| {
            path.join(allowed_dirs_filepath.unwrap_or_else(|| DEFAULT_ALLOWED_DIRS_FILENAME.into()))
        })
    }

    fn config(
        &self,
        cfg_filepaths: Vec<PathBuf>,
        project_dirpath: Option<PathBuf>,
    ) -> Result<Vec<PathBuf>> {
        let project_dirpath = project_dirpath.map(Ok).unwrap_or_else(|| self.fs.cwd())?;
        if cfg_filepaths.is_empty() {
            let mut cfg_filepaths = vec![];
            let default_cfg_filepath = project_dirpath.join(PROJECT_CONFIG_FILENAME);
            let local_cfg_filepath = project_dirpath.join(LOCAL_CONFIG_FILENAME);
            if default_cfg_filepath.exists() {
                cfg_filepaths.push(default_cfg_filepath);
            }
            if local_cfg_filepath.exists() {
                cfg_filepaths.push(local_cfg_filepath);
            }
            Ok(cfg_filepaths)
        } else {
            Ok(cfg_filepaths)
        }
    }

    fn projectctl_dir(&self, projectctl_dirpath: Option<PathBuf>) -> Result<PathBuf> {
        projectctl_dirpath.map(Ok).unwrap_or_else(|| {
            self.fs
                .home_dirpath()
                .map(|path| path.join(DEFAULT_PROJECTCTL_DIRNAME))
        })
    }
}

#[cfg(test)]
mod test {
    use std::fs::File;

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
                projectctl_dirpath: Option<PathBuf>,
            }

            #[test]
            fn ok_when_no_override() {
                test(
                    |_| Parameters {
                        allowed_dirs_filepath: None,
                        projectctl_dirpath: None,
                    },
                    |ctx, res| {
                        assert(
                            res,
                            ctx.home_dirpath
                                .join(DEFAULT_PROJECTCTL_DIRNAME)
                                .join(DEFAULT_ALLOWED_DIRS_FILENAME),
                        );
                    },
                )
            }

            #[test]
            fn ok_when_projectctl_dirpath_is_overriden() {
                let projectctl_dirpath = tempdir().unwrap().into_path();
                test(
                    |_| Parameters {
                        allowed_dirs_filepath: None,
                        projectctl_dirpath: Some(projectctl_dirpath.clone()),
                    },
                    |_, res| {
                        assert(res, projectctl_dirpath.join(DEFAULT_ALLOWED_DIRS_FILENAME));
                    },
                )
            }

            #[test]
            fn ok_when_allowed_dirs_filepath_is_overriden() {
                let allowed_dirs_filepath = tempdir().unwrap().into_path().join("test");
                let projectctl_dirpath = tempdir().unwrap().into_path();
                test(
                    {
                        let allowed_dirs_filepath = allowed_dirs_filepath.clone();
                        move |_| Parameters {
                            allowed_dirs_filepath: Some(allowed_dirs_filepath.clone()),
                            projectctl_dirpath: Some(projectctl_dirpath.clone()),
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
                let res =
                    paths.allowed_dirs(params.allowed_dirs_filepath, params.projectctl_dirpath);
                assert_fn(&ctx, res);
            }
        }

        mod config {
            use super::*;

            struct Context {
                cwd: PathBuf,
            }

            struct Parameters {
                cfg_filepaths: Vec<PathBuf>,
                create_local_file: bool,
                create_project_file: bool,
                project_dirpath: Option<PathBuf>,
            }

            #[test]
            fn ok_when_no_override_and_files_dont_exist() {
                test(
                    |_| Parameters {
                        cfg_filepaths: vec![],
                        create_local_file: false,
                        create_project_file: false,
                        project_dirpath: None,
                    },
                    |_, res| assert(res, vec![]),
                )
            }

            #[test]
            fn ok_when_no_override_and_project_file_exists() {
                test(
                    |_| Parameters {
                        cfg_filepaths: vec![],
                        create_local_file: false,
                        create_project_file: true,
                        project_dirpath: None,
                    },
                    |ctx, res| assert(res, vec![ctx.cwd.join(PROJECT_CONFIG_FILENAME)]),
                )
            }

            #[test]
            fn ok_when_no_override_and_local_file_exists() {
                test(
                    |_| Parameters {
                        cfg_filepaths: vec![],
                        create_local_file: true,
                        create_project_file: false,
                        project_dirpath: None,
                    },
                    |ctx, res| assert(res, vec![ctx.cwd.join(LOCAL_CONFIG_FILENAME)]),
                )
            }

            #[test]
            fn ok_when_no_override_and_files_exist() {
                test(
                    |_| Parameters {
                        cfg_filepaths: vec![],
                        create_local_file: true,
                        create_project_file: true,
                        project_dirpath: None,
                    },
                    |ctx, res| {
                        assert(
                            res,
                            vec![
                                ctx.cwd.join(PROJECT_CONFIG_FILENAME),
                                ctx.cwd.join(LOCAL_CONFIG_FILENAME),
                            ],
                        )
                    },
                )
            }

            #[test]
            fn ok_when_cfg_filepaths_is_not_empty() {
                let root_dirpath = tempdir().unwrap().into_path();
                let cfg_filepaths = vec![root_dirpath.join("cfg1"), root_dirpath.join("cfg2")];
                test(
                    |_| Parameters {
                        cfg_filepaths: cfg_filepaths.clone(),
                        create_local_file: true,
                        create_project_file: true,
                        project_dirpath: None,
                    },
                    |_, res| assert(res, cfg_filepaths.clone()),
                )
            }

            #[test]
            fn ok_when_project_dirpath_is_defined_and_files_dont_exist() {
                let project_dirpath = tempdir().unwrap().into_path();
                test(
                    |_| Parameters {
                        cfg_filepaths: vec![],
                        create_local_file: false,
                        create_project_file: false,
                        project_dirpath: Some(project_dirpath.clone()),
                    },
                    |_, res| assert(res, vec![]),
                )
            }

            #[test]
            fn ok_when_project_dirpath_is_defined_and_project_file_exists() {
                let project_dirpath = tempdir().unwrap().into_path();
                test(
                    |_| Parameters {
                        cfg_filepaths: vec![],
                        create_local_file: false,
                        create_project_file: true,
                        project_dirpath: Some(project_dirpath.clone()),
                    },
                    |_, res| assert(res, vec![project_dirpath.join(PROJECT_CONFIG_FILENAME)]),
                )
            }

            #[test]
            fn ok_when_project_dirpath_is_defined_and_local_file_exists() {
                let project_dirpath = tempdir().unwrap().into_path();
                test(
                    |_| Parameters {
                        cfg_filepaths: vec![],
                        create_local_file: true,
                        create_project_file: false,
                        project_dirpath: Some(project_dirpath.clone()),
                    },
                    |_, res| assert(res, vec![project_dirpath.join(LOCAL_CONFIG_FILENAME)]),
                )
            }

            #[test]
            fn ok_when_project_dirpath_is_defined_and_files_exist() {
                let project_dirpath = tempdir().unwrap().into_path();
                test(
                    |_| Parameters {
                        cfg_filepaths: vec![],
                        create_local_file: true,
                        create_project_file: true,
                        project_dirpath: Some(project_dirpath.clone()),
                    },
                    |_, res| {
                        assert(
                            res,
                            vec![
                                project_dirpath.join(PROJECT_CONFIG_FILENAME),
                                project_dirpath.join(LOCAL_CONFIG_FILENAME),
                            ],
                        )
                    },
                )
            }

            #[test]
            fn ok_when_project_dirpath_is_defined_and_cfg_filepaths_is_not_empty() {
                let root_dirpath = tempdir().unwrap().into_path();
                let cfg_filepaths = vec![root_dirpath.join("cfg1"), root_dirpath.join("cfg2")];
                let project_dirpath = tempdir().unwrap().into_path();
                test(
                    |_| Parameters {
                        cfg_filepaths: cfg_filepaths.clone(),
                        create_local_file: true,
                        create_project_file: true,
                        project_dirpath: Some(project_dirpath.clone()),
                    },
                    |_, res| assert(res, cfg_filepaths.clone()),
                )
            }

            fn assert(res: Result<Vec<PathBuf>>, expected_paths: Vec<PathBuf>) {
                let paths = res.unwrap();
                assert_eq!(paths, expected_paths);
            }

            fn test<P: Fn(&Context) -> Parameters, A: Fn(&Context, Result<Vec<PathBuf>>)>(
                create_params_fn: P,
                assert_fn: A,
            ) {
                let ctx = Context {
                    cwd: tempdir().unwrap().into_path(),
                };
                let params = create_params_fn(&ctx);
                let project_dirpath = if let Some(ref path) = params.project_dirpath {
                    path
                } else {
                    &ctx.cwd
                };
                if params.create_local_file {
                    File::create(project_dirpath.join(LOCAL_CONFIG_FILENAME)).unwrap();
                }
                if params.create_project_file {
                    File::create(project_dirpath.join(PROJECT_CONFIG_FILENAME)).unwrap();
                }
                let fs = StubFileSystem::new().with_stub_of_cwd({
                    let cwd = ctx.cwd.clone();
                    move |_| Ok(cwd.clone())
                });
                let paths = DefaultPaths { fs: Box::new(fs) };
                let res = paths.config(params.cfg_filepaths, params.project_dirpath);
                assert_fn(&ctx, res);
            }
        }

        mod projectctl_dir {
            use super::*;

            struct Context {
                home_dirpath: PathBuf,
            }

            struct Parameters {
                projectctl_dirpath: Option<PathBuf>,
            }

            #[test]
            fn ok_when_no_override() {
                test(
                    |_| Parameters {
                        projectctl_dirpath: None,
                    },
                    |ctx, res| {
                        assert(res, ctx.home_dirpath.join(DEFAULT_PROJECTCTL_DIRNAME));
                    },
                )
            }

            #[test]
            fn ok_when_projectctl_dirpath_is_overriden() {
                let projectctl_dirpath = tempdir().unwrap().into_path();
                test(
                    |_| Parameters {
                        projectctl_dirpath: Some(projectctl_dirpath.clone()),
                    },
                    |_, res| {
                        assert(res, projectctl_dirpath.clone());
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
                let res = paths.projectctl_dir(params.projectctl_dirpath);
                assert_fn(&ctx, res);
            }
        }
    }
}
