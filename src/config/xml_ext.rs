use crate::util::StringExt;
use anyhow::*;
use extend::ext;
use itertools::Itertools;
use std::fmt;

#[ext(pub)]
impl<'a, 'b> roxmltree::Node<'a, 'b> {
    fn find_child_with_tag(&self, tag_name: &str) -> Result<Self>
    where
        Self: Sized,
    {
        self.children()
            .find(|child| child.tag_name().name() == tag_name)
            .with_context(|| anyhow!("node {} contained no child of type {}", self.tag_name().name(), tag_name,))
    }

    fn try_attribute(&self, key: &str) -> Result<&str> {
        self.attribute(key)
            .with_context(|| anyhow!("attribute '{}' missing from '{}'", key, self.tag_name().name()))
    }
}

#[derive(Debug)]
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

#[derive(Debug)]
pub struct XmlElement<'a, 'b>(roxmltree::Node<'a, 'b>);

impl<'a, 'b> fmt::Display for XmlElement<'a, 'b> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let children = self
            .children()
            .map(|child| format!("{}", child))
            .map(|x| x.lines().map(|line| format!("  {}", line)).join("\n"))
            .join("\n");

        write!(f, "{}{}</{}>", self.as_tag_string(), children, self.tag_name())
    }
}

#[derive(Debug)]
pub struct XmlText<'a, 'b>(roxmltree::Node<'a, 'b>);

impl<'a, 'b> fmt::Display for XmlText<'a, 'b> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "Text({})", self.0.text().unwrap_or_default())
    }
}

impl<'a, 'b> XmlNode<'a, 'b> {
    pub fn as_text(self) -> Result<XmlText<'a, 'b>> {
        match self {
            XmlNode::Text(text) => Ok(text),
            _ => Err(anyhow!("'{}' is not a text node", self)),
        }
    }

    pub fn as_element(self) -> Result<XmlElement<'a, 'b>> {
        match self {
            XmlNode::Element(element) => Ok(element),
            _ => Err(anyhow!("'{}' is not an element node", self)),
        }
    }
}

impl<'a, 'b> XmlText<'a, 'b> {
    pub fn text(&self) -> String {
        self.0.text().unwrap_or_default().trim_lines().trim_matches('\n').to_owned()
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
        self.child_elements()
            .find(|child| child.tag_name() == tagname)
            .with_context(|| anyhow!("child element '{}' missing from {}", tagname, self.as_tag_string()))
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
        self.0
            .attribute(key)
            .with_context(|| anyhow!("'{}' missing attribute '{}'", self.as_tag_string(), key))
    }

    pub fn only_child(&self) -> Result<XmlNode> {
        let mut children_iter = self.children();
        let only_child = children_iter
            .next()
            .context(anyhow!("'{}' had no children", self.as_tag_string()))?;
        if children_iter.next().is_some() {
            bail!("'{}' had more than one child", &self);
        }
        Ok(only_child)
    }

    pub fn only_child_element(&self) -> Result<XmlElement> {
        Ok(self.only_child()?.as_element()?)
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
