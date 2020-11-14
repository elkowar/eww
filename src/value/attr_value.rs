use anyhow::*;
use std::{collections::HashMap, iter::FromIterator};

use super::*;

/// A value assigned to an attribute in a widget.
/// This can be a primitive String that contains any amount of variable
/// references, as would be generated by the string "foo {{var}} bar".
#[derive(Serialize, Deserialize, Clone, PartialEq, derive_more::Into, derive_more::From, Default)]
pub struct AttrValue(Vec<AttrValueElement>);

impl fmt::Debug for AttrValue {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "AttrValue({:?})", self.0)
    }
}

impl IntoIterator for AttrValue {
    type IntoIter = std::vec::IntoIter<Self::Item>;
    type Item = AttrValueElement;

    fn into_iter(self) -> Self::IntoIter {
        self.0.into_iter()
    }
}

impl FromIterator<AttrValueElement> for AttrValue {
    fn from_iter<T: IntoIterator<Item = AttrValueElement>>(iter: T) -> Self {
        AttrValue(iter.into_iter().collect())
    }
}

impl AttrValue {
    pub fn from_primitive<T: Into<PrimitiveValue>>(v: T) -> Self {
        AttrValue(vec![AttrValueElement::Primitive(v.into())])
    }

    pub fn iter(&self) -> std::slice::Iter<AttrValueElement> {
        self.0.iter()
    }

    pub fn var_refs(&self) -> impl Iterator<Item = &VarName> {
        self.0.iter().filter_map(|x| x.as_var_ref())
    }

    pub fn resolve_one_level(self, variables: &HashMap<VarName, AttrValue>) -> AttrValue {
        self.into_iter()
            .flat_map(|entry| match entry {
                AttrValueElement::VarRef(var_name) => match variables.get(&var_name) {
                    Some(value) => value.0.clone(),
                    _ => vec![AttrValueElement::VarRef(var_name)],
                },
                _ => vec![entry],
            })
            .collect()
    }

    pub fn resolve_fully(self, variables: &HashMap<VarName, PrimitiveValue>) -> Result<PrimitiveValue> {
        self.into_iter()
            .map(|element| match element {
                AttrValueElement::Primitive(x) => Ok(x),
                AttrValueElement::VarRef(var_name) => variables
                    .get(&var_name)
                    .cloned()
                    .with_context(|| format!("Unknown variable '{}' referenced", var_name)),
            })
            .collect()
    }

    // TODO this could be a fancy Iterator implementation, ig
    pub fn parse_string(s: &str) -> AttrValue {
        let mut elements = Vec::new();

        let mut cur_word = "".to_owned();
        let mut cur_varref: Option<String> = None;
        let mut curly_count = 0;
        for c in s.chars() {
            if let Some(ref mut varref) = cur_varref {
                if c == '}' {
                    curly_count -= 1;
                    if curly_count == 0 {
                        elements.push(AttrValueElement::VarRef(VarName(std::mem::take(varref))));
                        cur_varref = None
                    }
                } else {
                    curly_count = 2;
                    varref.push(c);
                }
            } else if c == '{' {
                curly_count += 1;
                if curly_count == 2 {
                    if !cur_word.is_empty() {
                        elements.push(AttrValueElement::primitive(std::mem::take(&mut cur_word)));
                    }
                    cur_varref = Some(String::new())
                }
            } else {
                if curly_count == 1 {
                    cur_word.push('{');
                }
                curly_count = 0;
                cur_word.push(c);
            }
        }
        if let Some(unfinished_varref) = cur_varref.take() {
            elements.push(AttrValueElement::primitive(unfinished_varref));
        } else if !cur_word.is_empty() {
            elements.push(AttrValueElement::primitive(cur_word));
        }
        AttrValue(elements)
    }
}

#[derive(Clone, PartialEq, Serialize, Deserialize)]
pub enum AttrValueElement {
    Primitive(PrimitiveValue),
    VarRef(VarName),
}

impl fmt::Debug for AttrValueElement {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            AttrValueElement::Primitive(x) => write!(f, "Primitive({:?})", x),
            AttrValueElement::VarRef(x) => write!(f, "VarRef({:?})", x),
        }
    }
}

impl AttrValueElement {
    pub fn primitive(s: String) -> Self {
        AttrValueElement::Primitive(PrimitiveValue::from_string(s))
    }

    pub fn as_var_ref(&self) -> Option<&VarName> {
        match self {
            AttrValueElement::VarRef(x) => Some(&x),
            _ => None,
        }
    }

    pub fn as_primitive(&self) -> Option<&PrimitiveValue> {
        match self {
            AttrValueElement::Primitive(x) => Some(&x),
            _ => None,
        }
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use pretty_assertions::assert_eq;
    #[test]
    fn test_parse_string_or_var_ref_list() {
        let input = "{{foo}}{{bar}}b{}azb{a}z{{bat}}{}quok{{test}}";
        let output = AttrValue::parse_string(input);
        assert_eq!(
            output,
            AttrValue(vec![
                AttrValueElement::VarRef(VarName("foo".to_owned())),
                AttrValueElement::VarRef(VarName("bar".to_owned())),
                AttrValueElement::primitive("b{}azb{a}z".to_owned()),
                AttrValueElement::VarRef(VarName("bat".to_owned())),
                AttrValueElement::primitive("{}quok".to_owned()),
                AttrValueElement::VarRef(VarName("test".to_owned())),
            ]),
        )
    }
    #[test]
    fn test_parse_string_with_var_refs_attr_value() {
        assert_eq!(
            AttrValue(
                vec![
                    AttrValueElement::VarRef(VarName("var".to_owned())),
                    AttrValueElement::primitive("something".to_owned())
                ]
                .into()
            ),
            AttrValue::parse_string("{{var}}something")
        );
    }
}
