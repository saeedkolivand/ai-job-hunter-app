pub mod board_login;
pub mod boards;
pub mod engine;
pub mod http;
pub mod linkedin;
pub mod scrape_url;
pub mod types;

pub use engine::ScraperEngine;
pub use types::{BoardSearchInput, JobPosting};
