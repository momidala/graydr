pub mod config;
pub mod error;
pub mod handlers;
pub mod model;
pub mod routes;
pub mod store;

pub use error::AppError;

use std::sync::Arc;
use store::ModuleStore;
use config::ServerConfig;

#[derive(Clone)]
pub struct AppState {
    pub store: Arc<dyn ModuleStore>,
    pub config: Arc<ServerConfig>,
}
