use std::path::PathBuf;
use async_trait::async_trait;
use bytes::Bytes;
use crate::model::{ModuleCoord, ModuleMeta};
use super::{ModuleStore, StoreError};

pub struct FilesystemStore {
    data_dir: PathBuf,
}

impl FilesystemStore {
    pub fn new(data_dir: PathBuf) -> Self {
        Self { data_dir }
    }

    fn module_dir(&self, coord: &ModuleCoord) -> PathBuf {
        // Never use coord.to_string() — Display drops version.
        // Always build path from separate components.
        self.data_dir
            .join(&coord.org)
            .join(&coord.name)
            .join(coord.version.to_string())
    }
}

#[async_trait]
impl ModuleStore for FilesystemStore {
    async fn put_module(&self, coord: &ModuleCoord, content: Bytes, meta: &ModuleMeta) -> Result<(), StoreError> {
        let dir = self.module_dir(coord);
        let module_path = dir.join("module.gmod");
        let meta_path = dir.join("meta.json");

        // 409: coordinate already published
        if module_path.exists() {
            return Err(StoreError::AlreadyExists);
        }

        tokio::fs::create_dir_all(&dir).await?;

        // Atomic write: temp-then-rename for each file
        let tmp_module = dir.join("module.gmod.tmp");
        tokio::fs::write(&tmp_module, &content).await?;
        tokio::fs::rename(&tmp_module, &module_path).await?;

        let meta_json = serde_json::to_string(meta)?;
        let tmp_meta = dir.join("meta.json.tmp");
        tokio::fs::write(&tmp_meta, meta_json.as_bytes()).await?;
        tokio::fs::rename(&tmp_meta, &meta_path).await?;

        Ok(())
    }

    async fn get_content(&self, coord: &ModuleCoord) -> Result<Bytes, StoreError> {
        let path = self.module_dir(coord).join("module.gmod");
        if !path.exists() {
            return Err(StoreError::NotFound);
        }
        let bytes = tokio::fs::read(&path).await?;
        Ok(Bytes::from(bytes))
    }

    async fn get_meta(&self, coord: &ModuleCoord) -> Result<ModuleMeta, StoreError> {
        let path = self.module_dir(coord).join("meta.json");
        if !path.exists() {
            return Err(StoreError::NotFound);
        }
        let raw = tokio::fs::read_to_string(&path).await?;
        let meta: ModuleMeta = serde_json::from_str(&raw)?;
        Ok(meta)
    }
}
