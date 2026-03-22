use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};

/// Blob storage — files on disk, metadata tracked by caller.
pub struct BlobStore {
    root: PathBuf,
}

impl BlobStore {
    pub fn new(root: impl Into<PathBuf>) -> anyhow::Result<Self> {
        let root = root.into();
        std::fs::create_dir_all(&root)?;
        Ok(Self { root })
    }

    /// Store a blob. Returns (path, sha256_hex, size_bytes).
    pub fn store(
        &self,
        namespace: &str,
        data: &[u8],
        extension: &str,
    ) -> anyhow::Result<BlobRecord> {
        let ns_dir = self.root.join(namespace);
        std::fs::create_dir_all(&ns_dir)?;

        let id = uuid::Uuid::new_v4().to_string();
        let filename = if extension.is_empty() {
            id.clone()
        } else {
            format!("{id}.{extension}")
        };
        let path = ns_dir.join(&filename);

        std::fs::write(&path, data)?;

        let sha256 = hex_sha256(data);
        let relative_path = format!("{namespace}/{filename}");

        Ok(BlobRecord {
            id,
            path: relative_path,
            sha256,
            size_bytes: data.len() as u64,
        })
    }

    /// Read a blob by relative path.
    pub fn read(&self, relative_path: &str) -> anyhow::Result<Vec<u8>> {
        let full_path = self.root.join(relative_path);
        if !full_path.starts_with(&self.root) {
            anyhow::bail!("path traversal attempt blocked");
        }
        Ok(std::fs::read(full_path)?)
    }

    /// Delete a blob by relative path.
    pub fn delete(&self, relative_path: &str) -> anyhow::Result<bool> {
        let full_path = self.root.join(relative_path);
        if !full_path.starts_with(&self.root) {
            anyhow::bail!("path traversal attempt blocked");
        }
        if full_path.exists() {
            std::fs::remove_file(full_path)?;
            Ok(true)
        } else {
            Ok(false)
        }
    }

    pub fn root(&self) -> &Path {
        &self.root
    }
}

pub struct BlobRecord {
    pub id: String,
    pub path: String,
    pub sha256: String,
    pub size_bytes: u64,
}

fn hex_sha256(data: &[u8]) -> String {
    let hash = Sha256::digest(data);
    hash.iter().fold(String::new(), |mut s, b| {
        use std::fmt::Write;
        write!(s, "{b:02x}").unwrap();
        s
    })
}
