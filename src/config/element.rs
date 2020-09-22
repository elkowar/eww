use super::*;

use crate::value::AttrValue;
use hocon_ext::HoconExt;
use std::collections::HashMap;
use std::convert::TryFrom;

#[derive(Debug, Clone, PartialEq)]
pub struct WidgetDefinition {
    pub name: String,
    pub structure: ElementUse,
    pub size: Option<(i32, i32)>,
}

impl WidgetDefinition {
    pub fn parse_hocon(name: String, hocon: &Hocon) -> Result<Self> {
        let definition = hocon.as_hash()?;
        let structure = definition
            .get("structure")
            .cloned()
            .context("structure must be set in widget definition")
            .and_then(ElementUse::parse_hocon)?;

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
}

#[derive(Debug, Clone, PartialEq)]
pub enum ElementUse {
    Widget(WidgetUse),
    Text(AttrValue),
}

impl ElementUse {
    pub fn parse_hocon(hocon: Hocon) -> Result<Self> {
        match hocon {
            Hocon::String(s) => Ok(ElementUse::Text(AttrValue::from_string(s))),
            Hocon::Hash(hash) => WidgetUse::parse_hocon_hash(hash).map(ElementUse::Widget),
            _ => Err(anyhow!("'{:?}' is not a valid element", hocon)),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Default)]
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

    pub fn parse_hocon_hash(data: HashMap<String, Hocon>) -> Result<WidgetUse> {
        let (widget_name, widget_config) = data.into_iter().next().unwrap();
        let widget_config = widget_config.as_hash().unwrap();

        // TODO allow for `layout_horizontal: [ elements ]` shorthand

        let children = match &widget_config.get("children") {
            Some(Hocon::String(text)) => Ok(vec![ElementUse::Text(AttrValue::from_string(text.to_string()))]),
            Some(Hocon::Array(children)) => children
                .clone()
                .into_iter()
                .map(ElementUse::parse_hocon)
                .collect::<Result<Vec<_>>>(),
            None => Ok(Vec::new()),
            _ => Err(anyhow!(
                "children must be either a list of elements or a string, but was '{:?}'"
            )),
        }?;

        let attrs = widget_config
            .into_iter()
            .filter_map(|(key, value)| Some((key.to_lowercase(), AttrValue::try_from(value).ok()?)))
            .collect();

        Ok(WidgetUse {
            name: widget_name.to_string(),
            children,
            attrs,
        })
    }

    pub fn get_attr(&self, key: &str) -> Result<&AttrValue> {
        self.attrs
            .get(key)
            .context(format!("attribute '{}' missing from widgetuse of '{}'", key, &self.name))
    }
}

impl From<WidgetUse> for ElementUse {
    fn from(other: WidgetUse) -> ElementUse {
        ElementUse::Widget(other)
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use maplit::hashmap;
    use pretty_assertions::assert_eq;

    #[test]
    fn test_parse_text() {
        assert_eq!(
            ElementUse::parse_hocon(Hocon::String("hi".to_string())).unwrap(),
            ElementUse::Text(AttrValue::Concrete(PrimitiveValue::String("hi".to_string())))
        );
    }

    #[test]
    fn test_parse_widget_use() {
        let input_complex = r#"{
            widget_name: {
                value: "test"
                children: [
                    { child: {} }
                    { child: {} }
                ]
            }
        }"#;
        let expected = WidgetUse {
            name: "widget_name".to_string(),
            children: vec![
                ElementUse::Widget(WidgetUse::new("child".to_string(), vec![])),
                ElementUse::Widget(WidgetUse::new("child".to_string(), vec![])),
            ],
            attrs: hashmap! { "value".to_string() => AttrValue::Concrete(PrimitiveValue::String("test".to_string()))},
        };
        assert_eq!(
            WidgetUse::parse_hocon_hash(parse_hocon(input_complex).unwrap().as_hash().unwrap().clone()).unwrap(),
            expected
        );
    }

    #[test]
    fn test_parse_widget_definition() {
        let input_complex = r#"{
            structure: { foo: {} }
        }"#;
        let expected = WidgetDefinition {
            name: "widget_name".to_string(),
            structure: ElementUse::Widget(WidgetUse::new("foo".to_string(), vec![])),
            size: None,
        };
        assert_eq!(
            WidgetDefinition::parse_hocon("widget_name".to_string(), &parse_hocon(input_complex).unwrap()).unwrap(),
            expected
        );
    }
}
