use std::path::PathBuf;

use crate::{
    consts::{CONFIG_DIRNAME, DEFAULT_ALLOWED_DIRS_FILENAME},
    err::Result,
    fs::FileSystem,
};

#[inline]
pub fn allowed_dirs_filepath(arg: Option<PathBuf>, fs: &dyn FileSystem) -> Result<PathBuf> {
    arg.map(Ok)
        .unwrap_or_else(|| config_dirpath(fs).map(|path| path.join(DEFAULT_ALLOWED_DIRS_FILENAME)))
}

#[inline]
pub fn config_dirpath(fs: &dyn FileSystem) -> Result<PathBuf> {
    fs.home_dirpath().map(|path| path.join(CONFIG_DIRNAME))
}
