use crate::value::PrimitiveValue;
use anyhow::*;
use element::*;
use hocon::*;
use hocon_ext::HoconExt;
use std::collections::HashMap;
use std::convert::TryFrom;

pub mod element;
pub mod hocon_ext;

#[derive(Debug, Clone)]
pub struct EwwConfig {
    widgets: HashMap<String, WidgetDefinition>,
    windows: HashMap<String, EwwWindowDefinition>,
    default_vars: HashMap<String, PrimitiveValue>,
}

#[allow(unused)]
macro_rules! try_type {
    ($typ:ty; $code:expr) => {{
        let x: $typ = try { $code };
        x
    }};
    ($typ:ty; $code:block) => {{
        let x: $typ = try { $code };
        x
    }};
}

impl EwwConfig {
    pub fn from_hocon(hocon: &Hocon) -> Result<EwwConfig> {
        let data = hocon.as_hash()?;

        let widgets = data
            .get("widgets")
            .context("widgets field missing")?
            .as_hash()?
            .iter()
            .map(|(n, def)| Ok((n.clone(), WidgetDefinition::parse_hocon(n.clone(), def)?)))
            .collect::<Result<_>>()?;

        let windows = data
            .get("windows")
            .context("windows field missing")?
            .as_hash()?
            .iter()
            .map(|(name, def)| Ok((name.clone(), EwwWindowDefinition::from_hocon(def)?)))
            .collect::<Result<_>>()?;

        let default_vars = data
            .get("default_vars")
            .unwrap_or(&Hocon::Hash(HashMap::new()))
            .as_hash()?
            .iter()
            .map(|(name, def)| Ok((name.clone(), PrimitiveValue::try_from(def)?)))
            .collect::<Result<_>>()?;

        Ok(EwwConfig {
            widgets,
            windows,
            default_vars,
        })
    }

    pub fn get_widgets(&self) -> &HashMap<String, WidgetDefinition> {
        &self.widgets
    }
    pub fn get_windows(&self) -> &HashMap<String, EwwWindowDefinition> {
        &self.windows
    }
    pub fn get_default_vars(&self) -> &HashMap<String, PrimitiveValue> {
        &self.default_vars
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct EwwWindowDefinition {
    pub position: (i32, i32),
    pub size: (i32, i32),
    pub widget: ElementUse,
}

impl EwwWindowDefinition {
    pub fn from_hocon(hocon: &Hocon) -> Result<EwwWindowDefinition> {
        let data = hocon.as_hash().context("window config has to be a map structure")?;
        let position: Option<_> = try {
            (
                data.get("pos")?.as_hash().ok()?.get("x")?.as_i64()? as i32,
                data.get("pos")?.as_hash().ok()?.get("y")?.as_i64()? as i32,
            )
        };
        let size: Option<_> = try {
            (
                data.get("size")?.as_hash().ok()?.get("x")?.as_i64()? as i32,
                data.get("size")?.as_hash().ok()?.get("y")?.as_i64()? as i32,
            )
        };

        let element = ElementUse::parse_hocon(data.get("widget").context("no widget use given")?.clone())?;

        Ok(EwwWindowDefinition {
            position: position.context("pos.x and pos.y need to be set")?,
            size: size.context("size.x and size.y need to be set")?,
            widget: element,
        })
    }
}

pub fn parse_hocon(s: &str) -> Result<Hocon> {
    let s = s.trim();
    Ok(HoconLoader::new().strict().load_str(s)?.hocon()?)
}
