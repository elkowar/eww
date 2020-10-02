use anyhow::*;
use extend::ext;

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
