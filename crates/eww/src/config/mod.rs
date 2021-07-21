use crate::{
    util,
    value::{PrimVal, VarName},
};

use anyhow::*;

use element::*;
use xml_ext::*;

pub mod element;
pub mod eww_config;
pub mod inbuilt;
pub mod script_var;
pub mod system_stats;
pub mod window_definition;
pub mod window_geometry;
pub mod xml_ext;
pub use eww_config::*;
pub use script_var::*;
pub use window_definition::*;
pub use window_geometry::*;

#[macro_export]
macro_rules! ensure_xml_tag_is {
    ($element:ident, $name:literal) => {
        ensure!(
            $element.tag_name() == $name,
            anyhow!("{} | Tag needed to be of type '{}', but was: {}", $element.text_pos(), $name, $element.as_tag_string())
        )
    };
}
