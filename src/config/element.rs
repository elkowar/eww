use super::*;

use crate::value::AttrValue;
use hocon_ext::HoconExt;
use maplit::hashmap;
use std::collections::HashMap;
use std::convert::TryFrom;

#[derive(Debug, Clone, PartialEq)]
pub struct WidgetDefinition {
    pub name: String,
    pub structure: WidgetUse,
    pub size: Option<(i32, i32)>,
}

impl WidgetDefinition {
    pub fn from_xml(xml: roxmltree::Node) -> Result<Self> {
        if !xml.is_element() {
            bail!("Tried to parse element of type {:?} as Widget definition", xml.node_type());
        } else if xml.tag_name().name().to_lowercase() != "def" {
            bail!(
                "Illegal element: only <def> may be used in definition block, but found '{}'",
                xml.tag_name().name()
            );
        } else if xml.children().count() != 1 {
            bail!(
                "Widget definition '{}' needs to contain exactly one element",
                xml.tag_name().name()
            );
        }

        Ok(WidgetDefinition {
            name: xml.try_attribute("name")?.to_owned(),

            size: if let Some(node) = xml.children().find(|child| child.tag_name().name() == "size") {
                Some((node.try_attribute("x")?.parse()?, node.try_attribute("y")?.parse()?))
            } else {
                None
            },

            // we can unwrap here, because we previously verified that there is exactly one child
            structure: WidgetUse::from_xml(xml.first_child().unwrap())?,
        })
    }

    pub fn parse_hocon(name: String, hocon: &Hocon) -> Result<Self> {
        let definition = hocon.as_hash()?;
        let structure = definition
            .get("structure")
            .cloned()
            .context("structure must be set in widget definition")
            .and_then(WidgetUse::parse_hocon)?;

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

#[derive(Debug, Clone, PartialEq, Default)]
pub struct WidgetUse {
    pub name: String,
    pub children: Vec<WidgetUse>,
    pub attrs: HashMap<String, AttrValue>,
}

impl WidgetUse {
    pub fn from_xml(xml: roxmltree::Node) -> Result<Self> {
        match xml.node_type() {
            roxmltree::NodeType::Text => Ok(WidgetUse::simple_text(AttrValue::parse_string(
                xml.text()
                    .context("couldn't get text from node")?
                    .trim_matches('\n')
                    .trim()
                    .to_owned(),
            ))),
            roxmltree::NodeType::Element => {
                let widget_name = xml.tag_name();
                let attrs = xml
                    .attributes()
                    .iter()
                    .map(|attr| (attr.name().to_owned(), AttrValue::parse_string(attr.value().to_owned())))
                    .collect::<HashMap<_, _>>();
                let children = xml
                    .children()
                    .filter(|child| !child.is_comment())
                    .filter(|child| !(child.is_text() && child.text().unwrap().trim().trim_matches('\n').is_empty()))
                    .map(|child| WidgetUse::from_xml(child))
                    .collect::<Result<_>>()?;
                Ok(WidgetUse {
                    name: widget_name.name().to_string(),
                    attrs,
                    children,
                })
            }
            _ => Err(anyhow!("Tried to parse node of type {:?} as widget use", xml.node_type())),
        }
    }
}

impl WidgetUse {
    pub fn new(name: String, children: Vec<WidgetUse>) -> Self {
        WidgetUse {
            name,
            children,
            attrs: HashMap::new(),
        }
    }

    pub fn parse_hocon(data: Hocon) -> Result<Self> {
        match data {
            Hocon::Hash(data) => {
                let (widget_name, widget_config) = data.into_iter().next().context("tried to parse empty hash as widget use")?;
                match widget_config {
                    Hocon::Hash(widget_config) => WidgetUse::from_hash_definition(widget_name.clone(), widget_config),
                    direct_childen => Ok(WidgetUse::new(
                        widget_name.clone(),
                        parse_widget_use_children(direct_childen)?,
                    )),
                }
            }
            primitive => Ok(WidgetUse::simple_text(AttrValue::try_from(&primitive)?)),
        }
    }

    /// generate a WidgetUse from an array-style definition
    /// i.e.: { layout: [ "hi", "ho" ] }
    pub fn from_array_definition(widget_name: String, children: Vec<Hocon>) -> Result<Self> {
        let children = children.into_iter().map(WidgetUse::parse_hocon).collect::<Result<_>>()?;
        Ok(WidgetUse::new(widget_name, children))
    }

    /// generate a WidgetUse from a hash-style definition
    /// i.e.: { layout: { orientation: "v", children: ["hi", "Ho"] } }
    pub fn from_hash_definition(widget_name: String, mut widget_config: HashMap<String, Hocon>) -> Result<Self> {
        let children = widget_config
            .remove("children")
            .map(parse_widget_use_children)
            .unwrap_or(Ok(Vec::new()))?;

        let attrs = widget_config
            .into_iter()
            .filter_map(|(key, value)| Some((key.to_lowercase(), AttrValue::try_from(&value).ok()?)))
            .collect();

        Ok(WidgetUse {
            name: widget_name.to_string(),
            children,
            attrs,
        })
    }

    pub fn simple_text(text: AttrValue) -> Self {
        WidgetUse {
            name: "label".to_owned(),
            children: vec![],
            attrs: hashmap! { "text".to_string() => text }, // TODO this hardcoded "text" is dumdum
        }
    }

    pub fn get_attr(&self, key: &str) -> Result<&AttrValue> {
        self.attrs
            .get(key)
            .context(format!("attribute '{}' missing from widgetuse of '{}'", key, &self.name))
    }
}

pub fn parse_widget_use_children(children: Hocon) -> Result<Vec<WidgetUse>> {
    match children {
        Hocon::Hash(_) => bail!(
            "children of a widget must either be a list of widgets or a primitive value, but got hash: {:?}",
            children
        ),
        Hocon::Array(widget_children) => widget_children
            .into_iter()
            .map(WidgetUse::parse_hocon)
            .collect::<Result<Vec<_>>>(),
        primitive => Ok(vec![WidgetUse::simple_text(AttrValue::try_from(&primitive)?)]),
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use maplit::hashmap;
    use pretty_assertions::assert_eq;

    #[test]
    fn test_parse_widget_use() {
        let input_complex = r#"{
            widget_name: {
                value: "test"
                children: [
                    { child: {} }
                    { child: { children: ["hi"] } }
                ]
            }
        }"#;
        let expected = WidgetUse {
            name: "widget_name".to_string(),
            children: vec![
                WidgetUse::new("child".to_string(), vec![]),
                WidgetUse::new(
                    "child".to_string(),
                    vec![WidgetUse::simple_text(AttrValue::Concrete(PrimitiveValue::String(
                        "hi".to_string(),
                    )))],
                ),
            ],
            attrs: hashmap! { "value".to_string() => AttrValue::Concrete(PrimitiveValue::String("test".to_string()))},
        };
        assert_eq!(
            WidgetUse::parse_hocon(parse_hocon(input_complex).unwrap().clone()).unwrap(),
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
            structure: WidgetUse::new("foo".to_string(), vec![]),
            size: None,
        };
        assert_eq!(
            WidgetDefinition::parse_hocon("widget_name".to_string(), &parse_hocon(input_complex).unwrap()).unwrap(),
            expected
        );
    }

    #[test]
    fn test_parse_widget_use_xml() {
        let input = r#"
        <widget_name attr1="hi" attr2="12">
            <child_widget/>
            foo
        </widget_name>
        "#;
        let document = roxmltree::Document::parse(input).unwrap();
        let xml = document.root_element().clone();

        let expected = WidgetUse {
            name: "widget_name".to_owned(),
            attrs: hashmap! {
                "attr1".to_owned() => AttrValue::Concrete(PrimitiveValue::String("hi".to_owned())),
                "attr2".to_owned() => AttrValue::Concrete(PrimitiveValue::Number(12f64)),
            },
            children: vec![
                WidgetUse::new("child_widget".to_owned(), Vec::new()),
                WidgetUse::simple_text(AttrValue::Concrete(PrimitiveValue::String("foo".to_owned()))),
            ],
        };
        assert_eq!(expected, WidgetUse::from_xml(xml).unwrap());
    }
}
