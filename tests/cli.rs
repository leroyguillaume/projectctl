use assert_cmd::Command;
use tempfile::tempdir;

const BIN_NAME: &str = env!("CARGO_PKG_NAME");

#[test]
fn new() {
    let name = "test";
    let cwd = tempdir().unwrap().into_path();
    Command::cargo_bin(BIN_NAME)
        .unwrap()
        .current_dir(&cwd)
        .arg("-vvv")
        .arg("new")
        .arg("-d")
        .arg("description=test")
        .arg("-d")
        .arg("owner=test")
        .arg("-d")
        .arg("repository-url=https://github.com/leroyguillaume/test")
        .arg("rs-simple")
        .arg(name)
        .assert()
        .success();
    assert!(cwd.join(name).is_dir());
}
