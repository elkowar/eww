pub mod attributes;
pub mod backend_window_options;
pub mod config;
pub mod file_provider;
pub mod script_var_definition;
#[cfg(test)]
mod test;
pub mod validate;
pub mod var_definition;
pub mod widget_definition;
pub mod widget_use;
pub mod window_definition;
pub mod window_geometry;

pub use config::*;
