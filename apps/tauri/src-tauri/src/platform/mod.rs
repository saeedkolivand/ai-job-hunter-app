pub mod chrome;
pub mod config;
pub mod process;

pub use chrome::detect_system_chrome;
pub use process::{cli_path, NoWindow};
