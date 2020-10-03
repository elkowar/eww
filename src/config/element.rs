use super::*;
use itertools::Itertools;

use crate::value::AttrValue;
use crate::with_text_pos_context;
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
                    "Illegal element: only <def> may be used in definition block, but found '{}'",
                    xml.as_tag_string()
                );
            }

            let size: Option<_> = try {
                (xml.attr("width").ok()?, xml.attr("height").ok()?)
            };
            let size: Option<Result<_>> = size.map(|(x, y)| Ok((x.parse()?, y.parse()?)));

            WidgetDefinition {
                name: xml.attr("name")?.to_owned(),
                size: size.transpose()?,
                structure: WidgetUse::from_xml_node(xml.only_child()?)?,
            }
        }
    }
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct WidgetUse {
    pub name: String,
    pub children: Vec<WidgetUse>,
    pub attrs: HashMap<String, AttrValue>,
}

impl WidgetUse {
    pub fn new(name: String, children: Vec<WidgetUse>) -> Self {
        WidgetUse {
            name,
            children,
            attrs: HashMap::new(),
        }
    }
    pub fn from_xml_node(xml: XmlNode) -> Result<Self> {
        match xml {
            XmlNode::Text(text) => Ok(WidgetUse::simple_text(AttrValue::Concrete(PrimitiveValue::String(
                text.text(),
            )))),
            XmlNode::Element(elem) => Ok(WidgetUse {
                name: elem.tag_name().to_string(),
                children: with_text_pos_context! { elem => elem.children().map(WidgetUse::from_xml_node).collect::<Result<_>>()?}?,
                attrs: elem
                    .attributes()
                    .iter()
                    .map(|attr| (attr.name().to_owned(), AttrValue::parse_string(attr.value().to_owned())))
                    .collect::<HashMap<_, _>>(),
            }),
            XmlNode::Ignored(_) => Err(anyhow!("Failed to parse node {:?} as widget use", xml))?,
        }
    }

    pub fn simple_text(text: AttrValue) -> Self {
        WidgetUse {
            name: "label".to_owned(),
            children: vec![],
            attrs: hashmap! { "text".to_string() => text }, // TODO this hardcoded "text" is dumdum
        }
    }

    // TODO Even just thinking of this gives me horrible nightmares.....
    //pub fn from_text(text: String) -> Self {
    //WidgetUse::text_with_var_refs(
    //text.split(" ")
    //.map(|word| AttrValue::parse_string(word.to_owned()))
    //.collect_vec(),
    //)
    //}

    //pub fn text_with_var_refs(elements: Vec<AttrValue>) -> Self {
    //dbg!(WidgetUse {
    //name: "layout".to_owned(),
    //attrs: hashmap! {
    //"halign".to_owned() => AttrValue::Concrete(PrimitiveValue::String("center".to_owned())),
    //"space-evenly".to_owned() => AttrValue::Concrete(PrimitiveValue::String("false".to_owned())),
    //},
    //children: elements.into_iter().map(WidgetUse::simple_text).collect(),
    //})
    //}

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

    fn mk_attr_str(s: &str) -> AttrValue {
        AttrValue::Concrete(PrimitiveValue::String(s.to_owned()))
    }

    #[test]
    fn test_simple_text() {
        let expected_attr_value = AttrValue::Concrete(PrimitiveValue::String("my text".to_owned()));
        let widget = WidgetUse::simple_text(expected_attr_value.clone());
        assert_eq!(
            widget,
            WidgetUse {
                name: "label".to_owned(),
                children: Vec::new(),
                attrs: hashmap! { "text".to_owned() => expected_attr_value},
            },
        )
    }

    #[test]
    fn test_text_with_var_refs() {
        let expected_attr_value1 = mk_attr_str("my text");
        let expected_attr_value2 = AttrValue::VarRef("var".to_owned());
        let widget = WidgetUse::text_with_var_refs(vec![expected_attr_value1.clone(), expected_attr_value2.clone()]);
        assert_eq!(
            widget,
            WidgetUse {
                name: "layout".to_owned(),
                attrs: hashmap! { "halign".to_owned() => mk_attr_str("center"), "space-evenly".to_owned() => mk_attr_str("false")},
                children: vec![
                    WidgetUse::simple_text(expected_attr_value1),
                    WidgetUse::simple_text(expected_attr_value2),
                ]
            }
        )
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

        println!("{}", xml);
        assert_eq!(true, false);

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
                children: vec![WidgetUse::simple_text(AttrValue::Concrete(PrimitiveValue::String(
                    "test".to_owned(),
                )))],
                attrs: HashMap::new(),
            },
        };

        assert_eq!(
            expected,
            WidgetDefinition::from_xml_element(xml.as_element().unwrap()).unwrap()
        );
    }
}
