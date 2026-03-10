pub mod error;
pub mod filesystem;

pub use error::StoreError;
pub use filesystem::FilesystemStore;

use async_trait::async_trait;
use crate::model::{ModuleCoord, ModuleMeta, LifecycleState, VersionEntry};

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

    async fn update_lifecycle(
        &self,
        coord: &ModuleCoord,
        new_state: LifecycleState,
    ) -> Result<(), StoreError>;

    async fn list_versions(&self, org: &str, name: &str) -> Result<Vec<VersionEntry>, StoreError>;
}
