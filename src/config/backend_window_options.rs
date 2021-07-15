use crate::config::xml_ext::XmlElement;
use anyhow::*;

pub use backend::*;

#[cfg(feature = "x11")]
mod backend {

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

            let window_type = xml.parse_optional_attr("windowtype")?;

            Ok(BackendWindowOptions {
                wm_ignore: xml.parse_optional_attr("wm-ignore")?.unwrap_or(window_type.is_none() && struts.is_none()),
                window_type: window_type.unwrap_or_default(),
                sticky: xml.parse_optional_attr("sticky")?.unwrap_or(true),
                struts: struts.unwrap_or_default(),
            })
        }
    }
}

#[cfg(feature = "wayland")]
mod backend {
    use super::*;
    #[derive(Debug, Clone, PartialEq, Eq)]
    pub struct BackendWindowOptions {
        pub exclusive: bool,
        pub focusable: bool,
    }
    impl BackendWindowOptions {
        pub fn from_xml_element(xml: &XmlElement) -> Result<Self> {
            Ok(BackendWindowOptions {
                exclusive: xml.parse_optional_attr("exclusive")?.unwrap_or(false),
                focusable: xml.parse_optional_attr("focusable")?.unwrap_or(false),
            })
        }
    }
}

#[cfg(feature = "no-x11-wayland")]
mod backend {
    use super::*;
    #[derive(Debug, Clone, PartialEq, Eq)]
    pub struct BackendWindowOptions;
    impl BackendWindowOptions {
        pub fn from_xml_element(xml: &XmlElement) -> Result<Self> {
            Ok(BackendWindowOptions)
        }
    }
}
