pub mod chrome;
pub mod config;
pub mod process;

pub use chrome::{detect_chromium_user_data_roots, detect_system_chrome, ChromiumBrowser};
pub use process::{cli_path, NoWindow};
