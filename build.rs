use std::path::Path;

use git2::{Error, Repository};
use regex::Regex;

const PKG_VERSION: &str = env!("CARGO_PKG_VERSION");

const TAG_PATTERN: &str = "^v([0-9]+\\.[0-9]+\\.[0-9]+)";

fn main() {
    println!("cargo:rerun-if-changed=.git/**");
    match version() {
        Ok(version) => set_version(version),
        Err(err) => {
            let version = dev_version();
            println!("cargo:warning=Unable to compute version");
            println!("cargo:warning=git: {}", err);
            println!("cargo:warning=Setting VERSION to `{}`", version);
            set_version(version);
        }
    }
}

fn set_version(version: String) {
    println!("cargo:rustc-env=VERSION={}", version)
}

fn dev_version() -> String {
    format!("{}+dev", PKG_VERSION)
}

fn version() -> Result<String, Error> {
    let manifest_dirpath = Path::new(env!("CARGO_MANIFEST_DIR"));
    let repo = Repository::open(manifest_dirpath)?;
    let diff = repo.diff_index_to_workdir(None, None)?;
    if diff.deltas().count() > 0 {
        Ok(dev_version())
    } else {
        let tag_regex = Regex::new(TAG_PATTERN).unwrap();
        let head_commit_id = repo.head()?.peel_to_commit()?.id();
        let tag_names = repo.tag_names(None)?;
        let captures = tag_names.iter().flatten().find_map(|tag_name| {
            tag_regex.captures(tag_name).and_then(|captures| {
                match repo.find_reference(&format!("refs/tags/{}", tag_name)) {
                    Ok(tag) => tag.target().and_then(|target_id| {
                        if target_id == head_commit_id {
                            Some(captures)
                        } else {
                            None
                        }
                    }),
                    Err(err) => {
                        println!("cargo:warning=Unable to fetch `{}` tag", tag_name);
                        println!("cargo:warning=git: {}", err);
                        None
                    }
                }
            })
        });
        if let Some(captures) = captures {
            let version = captures.get(1).unwrap().as_str();
            Ok(version.into())
        } else {
            let sha = head_commit_id.to_string();
            Ok(format!("{}+{}", PKG_VERSION, &sha[0..7]))
        }
    }
}
