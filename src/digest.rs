use std::{fs::File, io::Read, path::Path};

use sha2::{Digest, Sha256};
use tracing::debug;

use crate::model::ProjectctlResult;

pub fn hash<BYTES: AsRef<[u8]>>(bytes: BYTES) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    let checksum = hasher.finalize();
    hex::encode(checksum)
}

pub fn hash_file<PATH: AsRef<Path>>(path: PATH) -> ProjectctlResult<String> {
    debug!(path = %path.as_ref().display(), "computing checksum");
    let mut file = File::open(path)?;
    let mut bytes = [0u8; 512];
    let mut hasher = Sha256::new();
    loop {
        let read_count = file.read(&mut bytes)?;
        if read_count > 0 {
            hasher.update(&bytes[0..read_count]);
        } else {
            let checksum = hasher.finalize();
            break Ok(hex::encode(checksum));
        }
    }
}
