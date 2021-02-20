use crate::{
    config::element::{WidgetDefinition, WidgetUse},
    value::{AttrName, AttrValue, VarName},
};
use anyhow::*;
use dyn_clone;
use std::collections::HashMap;
pub trait WidgetNode: std::fmt::Debug + dyn_clone::DynClone + Send + Sync {
    fn get_name(&self) -> &str;
    fn get_text_pos(&self) -> Option<&roxmltree::TextPos>;
    fn get_children(&self) -> &Vec<Box<dyn WidgetNode>>;
    fn render(&self) -> Result<gtk::Widget>;
}

dyn_clone::clone_trait_object!(WidgetNode);

#[derive(Debug, Clone)]
pub struct UserDefined {
    name: String,
    text_pos: Option<roxmltree::TextPos>,
    content: Box<dyn WidgetNode>,
}

impl WidgetNode for UserDefined {
    fn get_name(&self) -> &str {
        &self.name
    }

    fn get_text_pos(&self) -> Option<&roxmltree::TextPos> {
        self.text_pos.as_ref()
    }

    fn get_children(&self) -> &Vec<Box<dyn WidgetNode>> {
        self.content.get_children()
    }

    fn render(&self) -> Result<gtk::Widget> {
        self.content.render()
    }
}

#[derive(Debug, Clone)]
pub struct Generic {
    name: String,
    text_pos: Option<roxmltree::TextPos>,
    children: Vec<Box<dyn WidgetNode>>,
    attrs: HashMap<AttrName, AttrValue>,
}

impl WidgetNode for Generic {
    fn get_name(&self) -> &str {
        &self.name
    }

    fn get_text_pos(&self) -> Option<&roxmltree::TextPos> {
        self.text_pos.as_ref()
    }

    fn get_children(&self) -> &Vec<Box<dyn WidgetNode>> {
        &self.children
    }

    fn render(&self) -> Result<gtk::Widget> {
        unimplemented!();
    }
}

pub fn generate_generic_widget_node(
    defs: &HashMap<String, WidgetDefinition>,
    local_env: &HashMap<VarName, AttrValue>,
    w: WidgetUse,
) -> Result<Box<dyn WidgetNode>> {
    if let Some(def) = defs.get(&w.name) {
        ensure!(w.children.is_empty(), "User-defined widgets cannot be given children.");

        let new_local_env = w
            .attrs
            .into_iter()
            .map(|(name, value)| (VarName(name.0), value.resolve_one_level(local_env)))
            .collect::<HashMap<_, _>>();

        let content = generate_generic_widget_node(defs, &new_local_env, def.structure.clone())?;
        Ok(Box::new(UserDefined {
            name: w.name,
            text_pos: w.text_pos,
            content,
        }))
    } else {
        Ok(Box::new(Generic {
            name: w.name,
            text_pos: w.text_pos,
            attrs: w
                .attrs
                .into_iter()
                .map(|(name, value)| (name, value.resolve_one_level(local_env)))
                .collect(),
            children: w
                .children
                .into_iter()
                .map(|child| generate_generic_widget_node(defs, local_env, child))
                .collect::<Result<Vec<_>>>()?,
        }))
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::config::xml_ext::*;
    use maplit::hashmap;
    use pretty_assertions::assert_eq;
    #[test]
    fn test_generic_generate() {
        let w_def1 = {
            let input = r#"<def name="foo"><box>{{nested1}}{{raw1}}</box></def>"#;
            let document = roxmltree::Document::parse(input).unwrap();
            let xml = XmlNode::from(document.root_element().clone());
            WidgetDefinition::from_xml_element(&xml.as_element().unwrap()).unwrap()
        };
        let w_def2 = {
            let input = r#"<def name="bar"><foo nested1="{{nested2}}" raw1="raw value"/></def>"#;
            let document = roxmltree::Document::parse(input).unwrap();
            let xml = XmlNode::from(document.root_element().clone());
            WidgetDefinition::from_xml_element(&xml.as_element().unwrap()).unwrap()
        };
        let w_use = {
            let input = r#"<bar nested2="{{in_root}}"/>"#;
            let document = roxmltree::Document::parse(input).unwrap();
            let xml = XmlNode::from(document.root_element().clone());
            WidgetUse::from_xml_node(xml).unwrap()
        };

        let generic = generate_generic_widget_node(
            &hashmap! { "foo".to_string() => w_def1, "bar".to_string() => w_def2 },
            &HashMap::new(),
            w_use,
        )
        .unwrap();

        assert_eq!(generic.get_name(), "box".to_string());

        dbg!(&generic);
        // panic!("REEEEEEEEEE")
    }
}
