use anyhow::*;
use std::{collections::HashMap, iter::FromIterator};

use super::*;

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, derive_more::Into, derive_more::From)]
pub struct AttrValue(Vec<StringOrVarRef>);

impl IntoIterator for AttrValue {
    type IntoIter = std::vec::IntoIter<Self::Item>;
    type Item = StringOrVarRef;

    fn into_iter(self) -> Self::IntoIter {
        self.0.into_iter()
    }
}

impl FromIterator<StringOrVarRef> for AttrValue {
    fn from_iter<T: IntoIterator<Item = StringOrVarRef>>(iter: T) -> Self {
        let mut result = AttrValue(Vec::new());
        result.0.extend(iter);
        result
    }
}

impl AttrValue {
    pub fn from_primitive<T: Into<PrimitiveValue>>(v: T) -> Self {
        AttrValue(vec![StringOrVarRef::Primitive(v.into())])
    }

    pub fn iter(&self) -> std::slice::Iter<StringOrVarRef> {
        self.0.iter()
    }

    pub fn var_refs(&self) -> impl Iterator<Item = &VarName> {
        self.0.iter().filter_map(|x| x.as_var_ref())
    }

    pub fn resolve_one_level(self, variables: &HashMap<VarName, AttrValue>) -> AttrValue {
        self.into_iter()
            .flat_map(|entry| match entry {
                StringOrVarRef::VarRef(var_name) => match variables.get(&var_name) {
                    Some(value) => value.0.clone(),
                    _ => vec![StringOrVarRef::VarRef(var_name)],
                },
                _ => vec![entry],
            })
            .collect()
    }

    pub fn resolve_fully(self, variables: &HashMap<VarName, PrimitiveValue>) -> Result<PrimitiveValue> {
        self.into_iter()
            .map(|element| match element {
                StringOrVarRef::Primitive(x) => Ok(x),
                StringOrVarRef::VarRef(var_name) => variables
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
                        elements.push(StringOrVarRef::VarRef(VarName(std::mem::take(varref))));
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
                            elements.push(StringOrVarRef::primitive(std::mem::take(&mut cur_word)));
                        }
                        cur_varref = Some(String::new())
                    }
                } else {
                    cur_word.push(c);
                }
            }
        }
        if let Some(unfinished_varref) = cur_varref.take() {
            elements.push(StringOrVarRef::primitive(unfinished_varref));
        } else if !cur_word.is_empty() {
            elements.push(StringOrVarRef::primitive(cur_word.to_owned()));
        }
        AttrValue(elements)
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum StringOrVarRef {
    Primitive(PrimitiveValue),
    VarRef(VarName),
}

impl StringOrVarRef {
    pub fn primitive(s: String) -> Self {
        StringOrVarRef::Primitive(PrimitiveValue::from_string(s))
    }

    pub fn as_var_ref(&self) -> Option<&VarName> {
        match self {
            StringOrVarRef::VarRef(x) => Some(&x),
            _ => None,
        }
    }

    pub fn as_primitive(&self) -> Option<&PrimitiveValue> {
        match self {
            StringOrVarRef::Primitive(x) => Some(&x),
            _ => None,
        }
    }
}

#[cfg(Test)]
mod test {
    use pretty_assertions::assert_eq;
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
    #[test]
    fn test_parse_string_with_var_refs_attr_value() {
        assert_eq!(
            AttrValue(
                vec![
                    StringOrVarRef::VarRef(VarName("var".to_owned())),
                    StringOrVarRef::primitive("something".to_owned())
                ]
                .into()
            ),
            AttrValue::parse_string("{{var}}something")
        );
    }
}
