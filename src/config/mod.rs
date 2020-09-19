use anyhow::*;
use hocon::*;
use hocon_ext::HoconExt;
use std::collections::HashMap;
use try_match::try_match;

pub mod hocon_ext;

#[derive(Debug, PartialEq)]
pub struct WidgetDefinition {
    pub name: String,
    pub structure: ElementUse,
}

#[derive(Debug, PartialEq)]
pub enum ElementUse {
    Widget(WidgetUse),
    Text(String),
}

#[derive(Debug, PartialEq)]
pub struct WidgetUse {
    pub name: String,
    pub children: Vec<ElementUse>,
    pub num_attrs: HashMap<String, f64>,
    pub str_attrs: HashMap<String, String>,
}

impl WidgetUse {
    pub fn new(name: String, children: Vec<ElementUse>) -> Self {
        WidgetUse {
            name,
            children,
            num_attrs: HashMap::new(),
            str_attrs: HashMap::new(),
        }
    }
}

impl From<WidgetUse> for ElementUse {
    fn from(other: WidgetUse) -> ElementUse {
        ElementUse::Widget(other)
    }
}

pub fn parse_widget_definition(text: &str) -> Result<WidgetDefinition> {
    let hocon = parse_hocon(text)?;

    let definition = hocon
        .as_hash()
        .ok_or_else(|| anyhow!("{:?} is not a hash", text))?;

    Ok(WidgetDefinition {
        name: definition["name"]
            .as_string()
            .context("name was not a string")?,
        structure: parse_element_use(definition.get("structure").unwrap().clone())?,
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

    let str_attrs: HashMap<String, String> = widget_config
        .into_iter()
        .filter_map(|(key, value)| {
            Some((
                key.clone(),
                try_match!(Hocon::String(x) = value).ok()?.clone(),
            ))
        })
        .collect();

    let num_attrs: HashMap<String, f64> = widget_config
        .iter()
        .filter_map(|(key, value)| {
            Some((
                key.to_string(),
                try_match!(Hocon::Integer(x) = value)
                    .map(|&x| x as f64)
                    .or_else(|_| try_match!(Hocon::Real(x) = value).map(|&x| x as f64))
                    .ok()?,
            ))
        })
        .collect();

    Ok(WidgetUse {
        name: widget_name.to_string(),
        children,
        str_attrs,
        num_attrs,
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

    #[test]
    fn test_parse_widget_definition() {
        let expected = WidgetDefinition {
            name: "example_widget".to_string(),
            structure: ElementUse::Widget(WidgetUse {
                name: "layout_horizontal".to_string(),
                children: vec![
                    ElementUse::Widget(WidgetUse::new(
                        "text".to_string(),
                        vec![ElementUse::Text("hi".to_string())],
                    )),
                    ElementUse::Widget(WidgetUse::new("text".to_string(), vec![])),
                ],
            }),
        };

        let parsed_hocon = parse_hocon("{ text: { children: \"hi\" } }").unwrap();
        assert_eq!(
            parse_element_use(parsed_hocon).unwrap(),
            ElementUse::Widget(WidgetUse::new(
                "text".to_string(),
                vec![ElementUse::Text("hi".to_string())]
            ))
        );
        assert_eq!(parse_widget_definition(EXAMPLE_CONFIG).unwrap(), expected);
    }
}
