//! emusic-server: Homelab self-hosted music server library.

#![forbid(unsafe_code)]

pub mod api;
pub mod auth;
pub mod config;
pub mod db;
pub mod scanner;
pub mod util;

pub use auth::TokenManager;
pub use config::Config;
pub use db::Database;
pub use util::security::RateLimiter;

pub struct AppState {
    pub db: Database,
    pub token_manager: TokenManager,
    pub rate_limiter: RateLimiter,
    pub config: Config,
}
