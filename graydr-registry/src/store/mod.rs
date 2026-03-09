pub mod error;
pub mod filesystem;

pub use error::StoreError;
pub use filesystem::FilesystemStore;

use async_trait::async_trait;
use crate::model::{ModuleCoord, ModuleMeta};

#[async_trait]
pub trait ModuleStore: Send + Sync {
    async fn put_module(
        &self,
        coord: &ModuleCoord,
        content: bytes::Bytes,
        meta: &ModuleMeta,
    ) -> Result<(), StoreError>;

    async fn get_content(&self, coord: &ModuleCoord) -> Result<bytes::Bytes, StoreError>;
    async fn get_meta(&self, coord: &ModuleCoord) -> Result<ModuleMeta, StoreError>;
}
