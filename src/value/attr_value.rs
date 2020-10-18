use anyhow::*;
use lazy_static::lazy_static;
use regex::Regex;

use super::*;

#[derive(Clone, Debug, PartialEq)]
pub enum AttrValue {
    Concrete(PrimitiveValue),
    StringWithVarRefs(StringWithVarRefs),
    VarRef(VarName),
}

impl AttrValue {
    pub fn as_string(&self) -> Result<String> {
        match self {
            AttrValue::Concrete(x) => x.as_string(),
            _ => Err(anyhow!("{:?} is not a string", self)),
        }
    }

    pub fn as_f64(&self) -> Result<f64> {
        match self {
            AttrValue::Concrete(x) => x.as_f64(),
            _ => Err(anyhow!("{:?} is not an f64", self)),
        }
    }

    pub fn as_i32(&self) -> Result<i32> {
        match self {
            AttrValue::Concrete(x) => x.as_i32(),
            _ => Err(anyhow!("{:?} is not an i32", self)),
        }
    }

    pub fn as_bool(&self) -> Result<bool> {
        match self {
            AttrValue::Concrete(x) => x.as_bool(),
            _ => Err(anyhow!("{:?} is not a bool", self)),
        }
    }

    pub fn as_var_ref(&self) -> Result<&VarName> {
        match self {
            AttrValue::VarRef(x) => Ok(x),
            _ => Err(anyhow!("{:?} is not a variable reference", self)),
        }
    }

    /// parses the value, trying to turn it into VarRef,
    /// a number and a boolean first, before deciding that it is a string.
    pub fn parse_string(s: String) -> Self {
        lazy_static! {
            static ref VAR_REF_PATTERN: Regex = Regex::new("\\{\\{(.*?)\\}\\}").unwrap();
        };

        let pattern: &Regex = &*VAR_REF_PATTERN;
        if let Some(match_range) = pattern.find(&s) {
            if match_range.start() == 0 && match_range.end() == s.len() {
                // we can unwrap here, as we just verified that there is a valid match already
                let ref_name = VAR_REF_PATTERN.captures(&s).and_then(|cap| cap.get(1)).unwrap().as_str();
                AttrValue::VarRef(VarName(ref_name.to_owned()))
            } else {
                AttrValue::StringWithVarRefs(StringWithVarRefs::parse_string(&s))
            }
        } else {
            AttrValue::Concrete(PrimitiveValue::from_string(s))
        }
    }
}
impl From<PrimitiveValue> for AttrValue {
    fn from(value: PrimitiveValue) -> Self {
        AttrValue::Concrete(value)
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use pretty_assertions::assert_eq;
    #[test]
    fn test_parse_concrete_attr_value() {
        assert_eq!(
            AttrValue::Concrete(PrimitiveValue::from_string("foo".to_owned())),
            AttrValue::parse_string("foo".to_owned())
        );
    }
    #[test]
    fn test_parse_var_ref_attr_value() {
        assert_eq!(
            AttrValue::VarRef(VarName("foo".to_owned())),
            AttrValue::parse_string("{{foo}}".to_owned())
        );
    }
    #[test]
    fn test_parse_string_with_var_refs_attr_value() {
        assert_eq!(
            AttrValue::StringWithVarRefs(
                vec![
                    StringOrVarRef::VarRef(VarName("var".to_owned())),
                    StringOrVarRef::primitive("something".to_owned())
                ]
                .into()
            ),
            AttrValue::parse_string("{{var}}something".to_owned())
        );
    }
}
