use crate::{config::{
        element::{WidgetDefinition, WidgetUse},
        xml_ext::TextPos,
        WindowName,
    }, eww_state::EwwState, value::{AttrName, AttrVal, PrimVal, VarName}};
use anyhow::*;
use dyn_clone;
use std::collections::HashMap;

pub trait WidgetNode: std::fmt::Debug + dyn_clone::DynClone + Send + Sync {
    fn get_name(&self) -> &str;
    fn get_text_pos(&self) -> Option<TextPos>;

    /// Generate a [gtk::Widget] from a [WidgetNode].
    ///
    /// Also registers all the necessary state-change handlers in the eww_state.
    ///
    /// This may return `Err` in case there was an actual error while parsing or
    /// resolving the widget, Or `Ok(None)` if the widget_use just didn't match any
    /// widget name.
    fn render(
        &self,
        eww_state: &mut EwwState,
        window_name: &WindowName,
        widget_definitions: &HashMap<String, WidgetDefinition>,
        // TODO overrides must now be local-context again,..... REEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEE
        // i fucking hate my life. why is everything so shit
        // WHY WHY WHY WHY WHY WHY WHY WHY WHY WHY WHY WHY WHY WHY WHY WHY
        // PAIN PAIN PAIN PAIN PAIN PAIN PAIN PAIN PAIN PAIN PAIN PAIN PAIN PAIN PAIN PAIN PAIN PAIN PAIN PAIN PAIN PAIN PAIN
        // PAIN PAIN PAIN PAIN PAIN PAIN PAIN PAIN PAIN PAIN PAIN PAIN PAIN PAIN PAIN PAIN PAIN PAIN PAIN PAIN PAIN PAIN PAIN
        // PAIN PAIN PAIN PAIN PAIN PAIN PAIN PAIN PAIN PAIN PAIN PAIN PAIN PAIN PAIN PAIN PAIN PAIN PAIN PAIN PAIN PAIN PAIN
        // PAIN PAIN PAIN PAIN PAIN PAIN PAIN PAIN PAIN PAIN PAIN PAIN PAIN PAIN PAIN PAIN PAIN PAIN PAIN PAIN PAIN PAIN PAIN
        // PAIN PAIN PAIN PAIN PAIN PAIN PAIN PAIN PAIN PAIN PAIN PAIN PAIN PAIN PAIN PAIN PAIN PAIN PAIN PAIN PAIN PAIN PAIN
        // PAIN PAIN PAIN PAIN PAIN PAIN PAIN PAIN PAIN PAIN PAIN PAIN PAIN PAIN PAIN PAIN PAIN PAIN PAIN PAIN PAIN PAIN PAIN
        // PAIN PAIN PAIN PAIN PAIN PAIN PAIN PAIN PAIN PAIN PAIN PAIN PAIN PAIN PAIN PAIN PAIN PAIN PAIN PAIN PAIN PAIN PAIN
        overrides: &HashMap<VarName, PrimVal>,
    ) -> Result<gtk::Widget>;
}

dyn_clone::clone_trait_object!(WidgetNode);

#[derive(Debug, Clone)]
pub struct UserDefined {
    name: String,
    text_pos: Option<TextPos>,
    content: Box<dyn WidgetNode>,
}

impl UserDefined {
    fn new(
        def: &WidgetDefinition,
        defs: &HashMap<String, WidgetDefinition>,
        local_env: &HashMap<VarName, AttrVal>,
        w: WidgetUse,
    ) -> Result<Self> {
        ensure!(w.children.is_empty(), "User-defined widgets cannot be given children.");

        let new_local_env = w
            .attrs
            .into_iter()
            .map(|(name, value)| (VarName(name.0), value.resolve_one_level(local_env)))
            .collect::<HashMap<_, _>>();

        let content = generate_generic_widget_node(defs, &new_local_env, def.structure.clone())?;
        Ok(UserDefined { name: w.name, text_pos: w.text_pos, content })
    }
}

impl WidgetNode for UserDefined {
    fn get_name(&self) -> &str {
        &self.name
    }

    fn get_text_pos(&self) -> Option<TextPos> {
        self.text_pos
    }

    fn render(
        &self,
        eww_state: &mut EwwState,
        window_name: &WindowName,
        widget_definitions: &HashMap<String, WidgetDefinition>,
        overrides: &HashMap<VarName, PrimVal>,
    ) -> Result<gtk::Widget> {
        self.content.render(eww_state, window_name, widget_definitions, overrides)
    }
}

#[derive(Debug, Clone)]
pub struct ForEach {
    pub text_pos: Option<TextPos>,
    pub child: Box<dyn WidgetNode>,
    pub elem_var_name_attr: VarName,
    pub iterable_attr: AttrVal,
}

impl ForEach {
    fn new(defs: &HashMap<String, WidgetDefinition>, local_env: &HashMap<VarName, AttrVal>, w: WidgetUse) -> Result<Self> {
        ensure!(
            w.children.len() == 1,
            "{}for widget needs to have exactly one child element",
            w.text_pos.map_or_else(Default::default, |x| format!("at {}: ", x))
        );
        let child = w.children[0];
        let elem_var_name_attr = VarName(w.attrs.get("each").context("<for> missing atribute \"each\"")?.to_string());
        let iterable_attr = w.attrs.get("in").context("<for> missing attribute \"in\"")?.resolve_one_level(local_env);
        let child_node = generate_generic_widget_node(defs, local_env, child)?;
        Ok(ForEach { text_pos: w.text_pos, child: child_node, elem_var_name_attr, iterable_attr })
    }
}

impl WidgetNode for ForEach {
    fn get_name(&self) -> &str {
        "for"
    }

    fn get_text_pos(&self) -> Option<TextPos> {
        self.text_pos
    }

    fn render(
        &self,
        eww_state: &mut EwwState,
        window_name: &WindowName,
        widget_definitions: &HashMap<String, WidgetDefinition>,
        overrides: &HashMap<VarName, PrimVal>,
    ) -> Result<gtk::Widget> {
        use gtk::ContainerExt;
        let outer_box = gtk::Box::new(gtk::Orientation::Vertical, 0);

        eww_state.resolve(
            window_name,
            maplit::hashmap! {
                "__for_iterable".into() => self.iterable_attr,
            },
            move |x| {
                let iterable_values = x
                    .get("__for_iterable")
                    .context("Missing __for_iterable, this is impossible!")?
                    .as_json_value()?
                    .as_array()
                    .context("<for> needs to be given an array")?;
                outer_box.get_children().iter().for_each(|w| gtk_widget.remove(w));

                for value in iterable_values {
                    self.child
                    let child = self.child.render()
                }
                outer_box.add()
                // reee
                Ok(())
            },
        );
        unimplemented!();

        // Ok(crate::widgets::build_builtin_gtk_widget(eww_state, window_name, widget_definitions, &self)?
        //.with_context(|| format!("Unknown widget '{}'", self.get_name()))?)
    }
}

#[derive(Debug, Clone)]
pub struct Generic {
    pub name: String,
    pub text_pos: Option<TextPos>,
    pub children: Vec<Box<dyn WidgetNode>>,
    pub attrs: HashMap<AttrName, AttrVal>,
}

impl Generic {
    pub fn new(defs: &HashMap<String, WidgetDefinition>, local_env: &HashMap<VarName, AttrVal>, w: WidgetUse) -> Result<Self> {
        Ok(Generic {
            name: w.name,
            text_pos: w.text_pos,
            attrs: w.attrs.into_iter().map(|(name, value)| (name, value.resolve_one_level(local_env))).collect(),
            children: w
                .children
                .into_iter()
                .map(|child| generate_generic_widget_node(defs, local_env, child))
                .collect::<Result<Vec<_>>>()?,
        })
    }

    pub fn get_attr(&self, key: &str) -> Result<&AttrVal> {
        self.attrs.get(key).context(format!("attribute '{}' missing from use of '{}'", key, &self.name))
    }

    /// returns all the variables that are referenced in this widget
    pub fn referenced_vars(&self) -> impl Iterator<Item = &VarName> {
        self.attrs.iter().flat_map(|(_, value)| value.var_refs())
    }
}

impl WidgetNode for Generic {
    fn get_name(&self) -> &str {
        &self.name
    }

    fn get_text_pos(&self) -> Option<TextPos> {
        self.text_pos
    }

    fn render(
        &self,
        eww_state: &mut EwwState,
        window_name: &WindowName,
        widget_definitions: &HashMap<String, WidgetDefinition>,
        overrides: &HashMap<VarName, PrimVal>,
    ) -> Result<gtk::Widget> {
        Ok(crate::widgets::build_builtin_gtk_widget(eww_state, window_name, widget_definitions, &self)?
            .with_context(|| format!("Unknown widget '{}'", self.get_name()))?)
    }
}

pub fn generate_generic_widget_node(
    defs: &HashMap<String, WidgetDefinition>,
    local_env: &HashMap<VarName, AttrVal>,
    w: WidgetUse,
) -> Result<Box<dyn WidgetNode>> {
    if let Some(def) = defs.get(&w.name) {
        Ok(Box::new(UserDefined::new(def, defs, local_env, w)?))
    } else if w.name == "for" {
        Ok(Box::new(ForEach::new(defs, local_env, w)?))
    } else {
        Ok(Box::new(Generic::new(defs, local_env, w)?))
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::config::xml_ext::*;
    use maplit::hashmap;
    #[test]
    fn test_generic_generate() {
        let w_def1 = {
            let input = r#"<def name="foo"><box><box>{{nested1}}{{raw1}}</box></box></def>"#;
            let document = roxmltree::Document::parse(input).unwrap();
            let xml = XmlNode::from(document.root_element().clone());
            WidgetDefinition::from_xml_element(&xml.as_element().unwrap()).unwrap()
        };
        let w_def2 = {
            let input = r#"<def name="bar"><box><foo nested1="{{nested2}}" raw1="raw value"/></box></def>"#;
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

        // TODO actually implement this test ._.

        dbg!(&generic);
        // panic!("REEEEEEEEEE")
    }
}
