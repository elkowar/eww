use anyhow::*;
use hocon::*;
use hocon_ext::HoconExt;
use std::collections::HashMap;
use std::convert::TryFrom;
use try_match::try_match;

pub mod hocon_ext;

#[derive(Debug, Clone)]
pub struct EwwConfig {
    widgets: HashMap<String, WidgetDefinition>,
    windows: HashMap<String, EwwWindowDefinition>,
    default_vars: HashMap<String, AttrValue>,
}

impl EwwConfig {
    pub fn from_hocon(hocon: &Hocon) -> Result<EwwConfig> {
        let data = hocon
            .as_hash()
            .context("eww config has to be a map structure")?;

        Ok(EwwConfig {
            widgets: data
                .get("widgets")
                .context("widgets need to be provided")?
                .as_hash()
                .context("widgets need to be a map")?
                .iter()
                .map(|(name, def)| Ok((name.clone(), parse_widget_definition(name.clone(), def)?)))
                .collect::<Result<HashMap<String, WidgetDefinition>>>()?,
            windows: data
                .get("windows")
                .context("windows need to be provided")?
                .as_hash()
                .context("windows need to be a map")?
                .iter()
                .map(|(name, def)| Ok((name.clone(), EwwWindowDefinition::from_hocon(def)?)))
                .collect::<Result<HashMap<String, EwwWindowDefinition>>>()?,
            default_vars: data
                .get("default_vars")
                .unwrap_or(&Hocon::Hash(HashMap::new()))
                .as_hash()
                .context("default_vars needs to be a map")?
                .iter()
                .map(|(name, def)| Ok((name.clone(), AttrValue::try_from(def)?)))
                .collect::<Result<HashMap<_, _>>>()?,
        })
    }

    pub fn get_widgets(&self) -> &HashMap<String, WidgetDefinition> {
        &self.widgets
    }
    pub fn get_windows(&self) -> &HashMap<String, EwwWindowDefinition> {
        &self.windows
    }
    pub fn get_default_vars(&self) -> &HashMap<String, AttrValue> {
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
        let data = hocon
            .as_hash()
            .context("window config has to be a map structure")?;
        let position: Option<_> = try {
            (
                data.get("pos")?.as_hash()?.get("x")?.as_i64()? as i32,
                data.get("pos")?.as_hash()?.get("y")?.as_i64()? as i32,
            )
        };
        let size: Option<_> = try {
            (
                data.get("size")?.as_hash()?.get("x")?.as_i64()? as i32,
                data.get("size")?.as_hash()?.get("y")?.as_i64()? as i32,
            )
        };

        let element =
            parse_element_use(data.get("widget").context("no widget use given")?.clone())?;

        Ok(EwwWindowDefinition {
            position: position.context("pos.x and pos.y need to be set")?,
            size: size.context("size.x and size.y need to be set")?,
            widget: element,
        })
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum AttrValue {
    String(String),
    Number(f64),
    Boolean(bool),
    VarRef(String),
}

impl AttrValue {
    pub fn as_string(&self) -> Option<&String> {
        try_match!(AttrValue::String(x) = self).ok()
    }
    pub fn as_f64(&self) -> Option<f64> {
        try_match!(AttrValue::Number(x) = self => *x).ok()
    }
    pub fn as_bool(&self) -> Option<bool> {
        try_match!(AttrValue::Boolean(x) = self => *x).ok()
    }
    pub fn as_var_ref(&self) -> Option<&String> {
        try_match!(AttrValue::VarRef(x) = self).ok()
    }
}

impl std::convert::TryFrom<&Hocon> for AttrValue {
    type Error = anyhow::Error;
    fn try_from(value: &Hocon) -> Result<Self> {
        Ok(match value {
            Hocon::String(s) if s.starts_with("$$") => {
                AttrValue::VarRef(s.trim_start_matches("$$").to_string())
            }
            Hocon::String(s) => AttrValue::String(s.to_string()),
            Hocon::Integer(n) => AttrValue::Number(*n as f64),
            Hocon::Real(n) => AttrValue::Number(*n as f64),
            Hocon::Boolean(b) => AttrValue::Boolean(*b),
            _ => return Err(anyhow!("cannot convert {} to config::AttrValue")),
        })
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct WidgetDefinition {
    pub name: String,
    pub structure: ElementUse,
    pub size: Option<(i32, i32)>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum ElementUse {
    Widget(WidgetUse),
    Text(String),
}

#[derive(Debug, Clone, PartialEq)]
pub struct WidgetUse {
    pub name: String,
    pub children: Vec<ElementUse>,
    pub attrs: HashMap<String, AttrValue>,
}

impl WidgetUse {
    pub fn new(name: String, children: Vec<ElementUse>) -> Self {
        WidgetUse {
            name,
            children,
            attrs: HashMap::new(),
        }
    }
}

impl From<WidgetUse> for ElementUse {
    fn from(other: WidgetUse) -> ElementUse {
        ElementUse::Widget(other)
    }
}

pub fn parse_widget_definition(name: String, hocon: &Hocon) -> Result<WidgetDefinition> {
    let definition = hocon
        .as_hash()
        .context("widget definition was not a hash")?;
    let structure = definition
        .get("structure")
        .cloned()
        .context("structure needs to be set")
        .and_then(parse_element_use)?;

    Ok(WidgetDefinition {
        name,
        structure,
        size: try {
            (
                definition.get("size_x")?.as_i64()? as i32,
                definition.get("size_y")?.as_i64()? as i32,
            )
        },
    })
}

pub fn parse_element_use(hocon: Hocon) -> Result<ElementUse> {
    match hocon {
        Hocon::String(s) => Ok(ElementUse::Text(s)),
        Hocon::Hash(hash) => parse_widget_use(hash).map(ElementUse::Widget),
        _ => Err(anyhow!("{:?} is not a valid element", hocon)),
    }
}

pub fn parse_widget_use(data: HashMap<String, Hocon>) -> Result<WidgetUse> {
    let (widget_name, widget_config) = data.into_iter().next().unwrap();
    let widget_config = widget_config.as_hash().unwrap();

    // TODO allow for `layout_horizontal: [ elements ]` shorthand

    let children = match &widget_config.get("children") {
        Some(Hocon::String(text)) => Ok(vec![ElementUse::Text(text.to_string())]),
        Some(Hocon::Array(children)) => children
            .clone()
            .into_iter()
            .map(parse_element_use)
            .collect::<Result<Vec<_>>>(),
        None => Ok(Vec::new()),
        _ => Err(anyhow!(
            "children must be either a list of elements or a string, but was {:?}"
        )),
    }?;

    let attrs: HashMap<String, AttrValue> = widget_config
        .into_iter()
        .filter_map(|(key, value)| Some((key.to_lowercase(), AttrValue::try_from(value).ok()?)))
        .collect();

    Ok(WidgetUse {
        name: widget_name.to_string(),
        children,
        attrs,
    })
}

pub fn parse_hocon(s: &str) -> Result<Hocon> {
    Ok(HoconLoader::new().load_str(s)?.hocon()?)
}

#[cfg(test)]
mod test {
    use super::*;

    const EXAMPLE_CONFIG: &'static str = r#"{
        name: "example_widget"
        structure {
            layout_horizontal {
                children: [
                    { text { children: "hi", color: "red" } }
                    { text: {} }
                ]
            }
        }
    }"#;

    #[test]
    fn test_parse() {
        assert_eq!(
            parse_element_use(Hocon::String("hi".to_string())).unwrap(),
            ElementUse::Text("hi".to_string())
        );
    }

    // #[test]
    // fn test_parse_widget_definition() {
    //     let expected = WidgetDefinition {
    //         name: "example_widget".to_string(),
    //         structure: ElementUse::Widget(WidgetUse {
    //             name: "layout_horizontal".to_string(),
    //             attrs: HashMap::new(),
    //             children: vec![
    //                 ElementUse::Widget(WidgetUse::new(
    //                     "text".to_string(),
    //                     vec![ElementUse::Text("hi".to_string())],
    //                 )),
    //                 ElementUse::Widget(WidgetUse::new("text".to_string(), vec![])),
    //             ],
    //         }),
    //     };

    //     let parsed_hocon = parse_hocon("{ text: { children: \"hi\" } }").unwrap();
    //     assert_eq!(
    //         parse_element_use(parsed_hocon).unwrap(),
    //         ElementUse::Widget(WidgetUse::new(
    //             "text".to_string(),
    //             vec![ElementUse::Text("hi".to_string())]
    //         ))
    //     );
    //     assert_eq!(parse_widget_definition(EXAMPLE_CONFIG).unwrap(), expected);
    // }
}
