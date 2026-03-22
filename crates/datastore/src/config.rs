/// Datastore runtime configuration — loaded from environment variables.
#[derive(Clone)]
pub struct DatastoreConfig {
    pub port: u16,
    pub db_path: String,
    pub blob_root: String,
    pub cloak_url: String,
    pub cloak_manifest_token: String,
}

impl DatastoreConfig {
    pub fn from_env() -> Result<Self, Box<dyn std::error::Error>> {
        let port = std::env::var("DATASTORE_PORT")
            .unwrap_or_else(|_| "8102".into())
            .parse()
            .unwrap_or(8102);

        let db_path = std::env::var("DATASTORE_DB_PATH")
            .unwrap_or_else(|_| "datastore.db".into());

        let blob_root = std::env::var("DATASTORE_BLOB_ROOT")
            .unwrap_or_else(|_| "/secure/blobs".into());

        let cloak_url = std::env::var("CLOAK_URL")
            .unwrap_or_else(|_| "http://localhost:8300".into());

        let cloak_manifest_token = std::env::var("CLOAK_MANIFEST_TOKEN")
            .unwrap_or_default();

        Ok(Self {
            port,
            db_path,
            blob_root,
            cloak_url,
            cloak_manifest_token,
        })
    }
}
