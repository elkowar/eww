use crate::util::StringExt;
use anyhow::*;
use itertools::Itertools;
use std::fmt;

#[macro_export]
macro_rules! with_text_pos_context {
    ($node:expr => $($code:tt)*) => {{
        let result: Result<_> = try { $($code)* };
        result.with_context(|| anyhow!("at: {}", $node.text_pos()))
    }};
}

#[derive(Debug, Clone)]
pub enum XmlNode<'a, 'b> {
    Element(XmlElement<'a, 'b>),
    Text(XmlText<'a, 'b>),
    Ignored(roxmltree::Node<'a, 'b>),
}

impl<'a, 'b> fmt::Display for XmlNode<'a, 'b> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            XmlNode::Text(text) => write!(f, "{}", text),
            XmlNode::Element(elem) => write!(f, "{}", elem),
            XmlNode::Ignored(node) => write!(f, "{:?}", node),
        }
    }
}

/// Get the part of a string that is selected by the start and end TextPos.
/// Will panic if the range is out of bounds in any way.
fn get_text_from_text_range(s: &str, (start_pos, end_pos): (roxmltree::TextPos, roxmltree::TextPos)) -> String {
    let mut code_text = s
        .lines()
        .dropping(start_pos.row as usize - 1)
        .take(end_pos.row as usize - (start_pos.row as usize - 1))
        .collect_vec();
    if let Some(first_line) = code_text.first_mut() {
        *first_line = first_line.split_at(start_pos.col as usize - 1).1;
    }
    if let Some(last_line) = code_text.last_mut() {
        *last_line = last_line.split_at(end_pos.col as usize - 1).0;
    }
    code_text.join("\n")
}

impl<'a, 'b> XmlNode<'a, 'b> {
    pub fn get_sourcecode(&self) -> String {
        let input_text = self.node().document().input_text();
        let range = self.node().range();
        let start_pos = self.node().document().text_pos_at(range.start);
        let end_pos = self.node().document().text_pos_at(range.end);
        get_text_from_text_range(input_text, (start_pos, end_pos))
    }

    pub fn as_text_or_sourcecode(&self) -> String {
        self.as_text().map(|c| c.text()).unwrap_or_else(|_| self.get_sourcecode())
    }

    pub fn as_text(&self) -> Result<&XmlText<'a, 'b>> {
        match self {
            XmlNode::Text(text) => Ok(text),
            _ => Err(anyhow!("'{}' is not a text node", self)),
        }
    }

    pub fn as_element(&self) -> Result<&XmlElement<'a, 'b>> {
        match self {
            XmlNode::Element(element) => Ok(element),
            _ => Err(anyhow!("'{}' is not an element node", self)),
        }
    }

    pub fn text_range(&self) -> std::ops::Range<usize> {
        self.node().range()
    }

    pub fn text_pos(&self) -> roxmltree::TextPos {
        let document = self.node().document();
        let range = self.node().range();
        document.text_pos_at(range.start)
    }

    fn node(&self) -> roxmltree::Node<'a, 'b> {
        match self {
            XmlNode::Text(x) => x.0,
            XmlNode::Element(x) => x.0,
            XmlNode::Ignored(x) => x.clone(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct XmlText<'a, 'b>(roxmltree::Node<'a, 'b>);

impl<'a, 'b> fmt::Display for XmlText<'a, 'b> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "Text(\"{}\")", self.text())
    }
}

impl<'a, 'b> XmlText<'a, 'b> {
    pub fn text(&self) -> String {
        self.0.text().unwrap_or_default().trim_lines().trim_matches('\n').to_owned()
    }

    pub fn text_pos(&self) -> roxmltree::TextPos {
        let document = self.0.document();
        let range = self.0.range();
        document.text_pos_at(range.start)
    }
}

#[derive(Debug, Clone)]
pub struct XmlElement<'a, 'b>(roxmltree::Node<'a, 'b>);

impl<'a, 'b> fmt::Display for XmlElement<'a, 'b> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let children = self
            .children()
            .map(|child| format!("{}", child))
            .map(|x| x.lines().map(|line| format!("  {}", line)).join("\n"))
            .join("\n");

        if children.len() == 0 {
            write!(f, "{}</{}>", self.as_tag_string(), self.tag_name())
        } else {
            write!(f, "{}\n{}\n</{}>", self.as_tag_string(), children, self.tag_name())
        }
    }
}

impl<'a, 'b> XmlElement<'a, 'b> {
    pub fn as_tag_string(&self) -> String {
        let attrs = self
            .attributes()
            .iter()
            .map(|attr| format!("{}=\"{}\"", attr.name(), attr.value()))
            .join(" ");

        format!("<{} {}>", self.tag_name(), attrs)
    }

    pub fn tag_name(&self) -> &str {
        self.0.tag_name().name()
    }

    pub fn child(&self, tagname: &str) -> Result<XmlElement> {
        with_text_pos_context! { self =>
            self.child_elements()
                .find(|child| child.tag_name() == tagname)
                .with_context(|| anyhow!("child element '{}' missing from {}", tagname, self.as_tag_string()))?
        }
    }

    pub fn children(&self) -> impl Iterator<Item = XmlNode> {
        self.0
            .children()
            .filter(|child| child.is_element() || (child.is_text() && !child.text().unwrap_or_default().is_blank()))
            .map(XmlNode::from)
    }

    pub fn child_elements(&self) -> impl Iterator<Item = XmlElement> {
        self.0.children().filter(|child| child.is_element()).map(XmlElement)
    }

    pub fn attributes(&self) -> &[roxmltree::Attribute] {
        self.0.attributes()
    }

    pub fn attr(&self, key: &str) -> Result<&str> {
        with_text_pos_context! { self =>
            self.0
                .attribute(key)
                .with_context(|| anyhow!("'{}' missing attribute '{}'", self.as_tag_string(), key))?
        }
    }

    pub fn only_child(&self) -> Result<XmlNode> {
        with_text_pos_context! { self =>
            let mut children_iter = self.children();
            let only_child = children_iter
                .next()
                .with_context(|| anyhow!("'{}' had no children", self.as_tag_string()))?;
            if children_iter.next().is_some() {
                bail!("'{}' had more than one child", &self);
            }
            only_child
        }
    }

    pub fn only_child_element(&self) -> Result<XmlElement> {
        with_text_pos_context! { self =>
            self.only_child()?.as_element()?.clone()
        }
    }

    pub fn text_pos(&self) -> roxmltree::TextPos {
        let document = self.0.document();
        let range = self.0.range();
        document.text_pos_at(range.start)
    }
}

impl<'a, 'b> From<XmlElement<'a, 'b>> for XmlNode<'a, 'b> {
    fn from(elem: XmlElement<'a, 'b>) -> Self {
        XmlNode::Element(elem)
    }
}

impl<'a, 'b> From<XmlText<'a, 'b>> for XmlNode<'a, 'b> {
    fn from(elem: XmlText<'a, 'b>) -> Self {
        XmlNode::Text(elem)
    }
}

impl<'a, 'b> From<roxmltree::Node<'a, 'b>> for XmlNode<'a, 'b> {
    fn from(node: roxmltree::Node<'a, 'b>) -> Self {
        if node.is_text() {
            XmlNode::Text(XmlText(node))
        } else if node.is_element() | node.is_root() {
            XmlNode::Element(XmlElement(node))
        } else {
            XmlNode::Ignored(node)
        }
    }
}
