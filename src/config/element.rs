use super::*;
use lazy_static::lazy_static;
use regex::Regex;
use std::ops::Range;

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

#[derive(Debug, Clone, Default)]
pub struct WidgetUse {
    pub name: String,
    pub children: Vec<WidgetUse>,
    pub attrs: HashMap<String, AttrValue>,
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
            // TODO the matching here is stupid. This currently uses the inefficient function to parse simple single varrefs,
            // TODO and does the regex match twice in the from_text_with_var_refs part
            XmlNode::Text(text) if PATTERN.is_match(&text.text()) => WidgetUse::from_text_with_var_refs(&text.text()),
            XmlNode::Text(text) => WidgetUse::simple_text(AttrValue::parse_string(text.text())),
            XmlNode::Element(elem) => WidgetUse {
                name: elem.tag_name().to_string(),
                children: with_text_pos_context! { elem => elem.children().map(WidgetUse::from_xml_node).collect::<Result<_>>()?}?,
                attrs: elem
                    .attributes()
                    .iter()
                    .map(|attr| (attr.name().to_owned(), AttrValue::parse_string(attr.value().to_owned())))
                    .collect::<HashMap<_, _>>(),
                ..WidgetUse::default()
            },
            XmlNode::Ignored(_) => Err(anyhow!("Failed to parse node {:?} as widget use", xml))?,
        };
        Ok(widget_use.at_pos(text_pos))
    }

    pub fn simple_text(text: AttrValue) -> Self {
        WidgetUse {
            name: "label".to_owned(),
            children: vec![],
            attrs: hashmap! { "text".to_string() => text }, // TODO this hardcoded "text" is dumdum
            ..WidgetUse::default()
        }
    }

    pub fn from_text_with_var_refs(text: &str) -> Self {
        WidgetUse {
            name: "layout".to_owned(),
            attrs: hashmap! {
                "halign".to_owned() => AttrValue::Concrete(PrimitiveValue::String("center".to_owned())),
                "space-evenly".to_owned() => AttrValue::Concrete(PrimitiveValue::String("false".to_owned())),
            },
            children: parse_string_with_var_refs(text)
                .into_iter()
                .map(StringOrVarRef::to_attr_value)
                .map(WidgetUse::simple_text)
                .collect(),
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

#[derive(Clone, Debug, PartialEq)]
enum StringOrVarRef {
    String(String),
    VarRef(String),
}

impl StringOrVarRef {
    fn to_attr_value(self) -> AttrValue {
        match self {
            StringOrVarRef::String(x) => AttrValue::Concrete(PrimitiveValue::parse_string(&x)),
            StringOrVarRef::VarRef(x) => AttrValue::VarRef(x),
        }
    }
}

// TODO this could be a fancy Iterator implementation, ig
fn parse_string_with_var_refs(s: &str) -> Vec<StringOrVarRef> {
    let mut elements = Vec::new();

    let mut cur_word = "".to_owned();
    let mut cur_varref: Option<String> = None;
    let mut curly_count = 0;
    for c in s.chars() {
        if let Some(ref mut varref) = cur_varref {
            if c == '}' {
                curly_count -= 1;
                if curly_count == 0 {
                    elements.push(StringOrVarRef::VarRef(std::mem::take(varref)));
                    cur_varref = None
                }
            } else {
                curly_count = 2;
                varref.push(c);
            }
        } else {
            if c == '{' {
                curly_count += 1;
                if curly_count == 2 {
                    if !cur_word.is_empty() {
                        elements.push(StringOrVarRef::String(std::mem::take(&mut cur_word)));
                    }
                    cur_varref = Some(String::new())
                }
            } else {
                cur_word.push(c);
            }
        }
    }
    if let Some(unfinished_varref) = cur_varref.take() {
        elements.push(StringOrVarRef::String(unfinished_varref));
    } else if !cur_word.is_empty() {
        elements.push(StringOrVarRef::String(cur_word.to_owned()));
    }
    elements
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
                ..WidgetUse::default()
            },
        )
    }

    #[test]
    fn test_text_with_var_refs() {
        let expected_attr_value1 = mk_attr_str("my text");
        let expected_attr_value2 = AttrValue::VarRef("var".to_owned());
        let widget = WidgetUse::from_text_with_var_refs("my text{{var}}");
        assert_eq!(
            widget,
            WidgetUse {
                name: "layout".to_owned(),
                attrs: hashmap! { "halign".to_owned() => mk_attr_str("center"), "space-evenly".to_owned() => mk_attr_str("false")},
                children: vec![
                    WidgetUse::simple_text(expected_attr_value1),
                    WidgetUse::simple_text(expected_attr_value2),
                ],
                ..WidgetUse::default()
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
                children: vec![WidgetUse::simple_text(AttrValue::Concrete(PrimitiveValue::String(
                    "test".to_owned(),
                )))],
                attrs: HashMap::new(),
                ..WidgetUse::default()
            },
        };

        assert_eq!(
            expected,
            WidgetDefinition::from_xml_element(xml.as_element().unwrap()).unwrap()
        );
    }

    #[test]
    fn test_parse_string_or_var_ref_list() {
        let input = "{{foo}}{{bar}}baz{{bat}}quok{{test}}";
        let output = parse_string_with_var_refs(input);
        assert_eq!(
            output,
            vec![
                StringOrVarRef::VarRef("foo".to_owned()),
                StringOrVarRef::VarRef("bar".to_owned()),
                StringOrVarRef::String("baz".to_owned()),
                StringOrVarRef::VarRef("bat".to_owned()),
                StringOrVarRef::String("quok".to_owned()),
                StringOrVarRef::VarRef("test".to_owned()),
            ],
        )
    }
}
