use crate::config::xml_ext::XmlElement;
use anyhow::*;

#[cfg(feature = "no-x11-wayland")]
pub use no_x11_wayland::*;
#[cfg(feature = "wayland")]
pub use wayland::*;
#[cfg(feature = "x11")]
pub use x11::*;

#[cfg(feature = "x11")]
mod x11 {

    use super::*;
    use crate::config::{EwwWindowType, StrutDefinition};

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub struct BackendWindowOptions {
        pub wm_ignore: bool,
        pub sticky: bool,
        pub window_type: EwwWindowType,
        pub struts: StrutDefinition,
    }

    impl BackendWindowOptions {
        pub fn from_xml_element(xml: &XmlElement) -> Result<Self> {
            let struts: Option<StrutDefinition> = xml
                .child("reserve")
                .ok()
                .map(StrutDefinition::from_xml_element)
                .transpose()
                .context("Failed to parse <reserve>")?;

            Ok(BackendWindowOptions {
                window_type: xml.parse_optional_attr("windowtype")?.unwrap_or_default(),
                wm_ignore: xml.parse_optional_attr("wm-ignore")?.unwrap_or(false),
                sticky: xml.parse_optional_attr("sticky")?.unwrap_or(true),
                struts: struts.unwrap_or_default(),
            })
        }
    }
}

#[cfg(feature = "wayland")]
mod wayland {
    use super::*;
    #[derive(Debug, Clone, PartialEq, Eq)]
    pub struct BackendWindowOptions {
        pub exclusive: bool,
    }
    impl BackendWindowOptions {
        pub fn from_xml_element(xml: &XmlElement) -> Result<Self> {
            Ok(BackendWindowOptions { exclusive: xml.parse_optional_attr("exclusive")?.unwrap_or(false) })
        }
    }
}

#[cfg(feature = "no-x11-wayland")]
mod no_x11_wayland {
    use super::*;
    #[derive(Debug, Clone, PartialEq, Eq)]
    pub struct BackendWindowOptions;
    impl BackendWindowOptions {
        pub fn from_xml_element(xml: &XmlElement) -> Result<Self> {
            Ok(BackendWindowOptions)
        }
    }
}
