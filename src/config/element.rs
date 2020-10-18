use super::*;
use lazy_static::lazy_static;
use regex::Regex;
use std::ops::Range;

use crate::{
    value::{AttrName, AttrValue},
    with_text_pos_context,
};
use maplit::hashmap;
use std::collections::HashMap;

#[derive(Debug, Clone, PartialEq)]
pub struct WidgetDefinition {
    pub name: String,
    pub structure: WidgetUse,
    pub size: Option<(i32, i32)>,
}

impl WidgetDefinition {
    pub fn from_xml_element(xml: XmlElement) -> Result<Self> {
        with_text_pos_context! { xml =>
            if xml.tag_name() != "def" {
                bail!(
                    "{} | Illegal element: only <def> may be used in definition block, but found '{}'",
                    xml.text_pos(),
                    xml.as_tag_string()
                );
            }

            let size: Option<_> = Option::zip(xml.attr("width").ok(), xml.attr("height").ok());
            let size: Option<Result<_>> = size.map(|(x, y)| Ok((x.parse()?, y.parse()?)));

            WidgetDefinition {
                name: xml.attr("name")?.to_owned(),
                size: size.transpose()?,
                structure: WidgetUse::from_xml_node(xml.only_child()?)?,
            }
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct WidgetUse {
    pub name: String,
    pub children: Vec<WidgetUse>,
    pub attrs: HashMap<AttrName, AttrValue>,
    pub text_pos: Option<roxmltree::TextPos>,
}

#[derive(Debug, Clone)]
pub struct PositionData {
    pub range: Range<usize>,
}

impl PartialEq for WidgetUse {
    fn eq(&self, other: &WidgetUse) -> bool {
        self.name == other.name && self.children == other.children && self.attrs == other.attrs
    }
}

impl WidgetUse {
    pub fn new(name: String, children: Vec<WidgetUse>) -> Self {
        WidgetUse {
            name,
            children,
            attrs: HashMap::new(),
            ..WidgetUse::default()
        }
    }

    pub fn from_xml_node(xml: XmlNode) -> Result<Self> {
        lazy_static! {
            static ref PATTERN: Regex = Regex::new("\\{\\{(.*)\\}\\}").unwrap();
        };
        let text_pos = xml.text_pos();
        let widget_use = match xml {
            XmlNode::Text(text) => WidgetUse::simple_text(AttrValue::parse_string(&text.text())),
            XmlNode::Element(elem) => WidgetUse {
                name: elem.tag_name().to_owned(),
                children: with_text_pos_context! { elem => elem.children().map(WidgetUse::from_xml_node).collect::<Result<_>>()?}?,
                attrs: elem
                    .attributes()
                    .iter()
                    .map(|attr| (AttrName(attr.name().to_owned()), AttrValue::parse_string(attr.value())))
                    .collect::<HashMap<_, _>>(),
                ..WidgetUse::default()
            },
            XmlNode::Ignored(_) => bail!("{} | Failed to parse node {:?} as widget use", xml.text_pos(), xml),
        };
        Ok(widget_use.at_pos(text_pos))
    }

    pub fn simple_text(text: AttrValue) -> Self {
        WidgetUse {
            name: "label".to_owned(),
            children: vec![],
            attrs: hashmap! { AttrName("text".to_owned()) => text }, // TODO this hardcoded "text" is dumdum
            ..WidgetUse::default()
        }
    }

    pub fn at_pos(mut self, text_pos: roxmltree::TextPos) -> Self {
        self.text_pos = Some(text_pos);
        self
    }

    pub fn get_attr(&self, key: &str) -> Result<&AttrValue> {
        self.attrs
            .get(key)
            .context(format!("attribute '{}' missing from widgetuse of '{}'", key, &self.name))
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use maplit::hashmap;
    use pretty_assertions::assert_eq;

    #[test]
    fn test_simple_text() {
        let expected_attr_value = AttrValue::from_primitive("my text");
        let widget = WidgetUse::simple_text(expected_attr_value.clone());
        assert_eq!(
            widget,
            WidgetUse {
                name: "label".to_owned(),
                children: Vec::new(),
                attrs: hashmap! { AttrName("text".to_owned()) => expected_attr_value},
                ..WidgetUse::default()
            },
        );
    }

    #[test]
    fn test_parse_widget_use() {
        let input = r#"
            <widget_name attr1="hi" attr2="12">
                <child_widget/>
                foo
            </widget_name>
        "#;
        let document = roxmltree::Document::parse(input).unwrap();
        let xml = XmlNode::from(document.root_element().clone());

        let expected = WidgetUse {
            name: "widget_name".to_owned(),
            attrs: hashmap! {
                 AttrName("attr1".to_owned()) => AttrValue::from_primitive("hi"),
                 AttrName("attr2".to_owned()) => AttrValue::from_primitive("12"),
            },
            children: vec![
                WidgetUse::new("child_widget".to_owned(), Vec::new()),
                WidgetUse::simple_text(AttrValue::from_primitive("foo".to_owned())),
            ],
            ..WidgetUse::default()
        };
        assert_eq!(expected, WidgetUse::from_xml_node(xml).unwrap());
    }

    #[test]
    fn test_parse_widget_definition() {
        let input = r#"
            <def name="foo" width="12" height="20">
                <layout>test</layout>
            </def>
        "#;
        let document = roxmltree::Document::parse(input).unwrap();
        let xml = XmlNode::from(document.root_element().clone());

        let expected = WidgetDefinition {
            name: "foo".to_owned(),
            size: Some((12, 20)),
            structure: WidgetUse {
                name: "layout".to_owned(),
                children: vec![WidgetUse::simple_text(AttrValue::from_primitive("test"))],
                attrs: HashMap::new(),
                ..WidgetUse::default()
            },
        };

        assert_eq!(
            expected,
            WidgetDefinition::from_xml_element(xml.as_element().unwrap().to_owned()).unwrap()
        );
    }
}
