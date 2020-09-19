use hocon::Hocon;
use std::collections::HashMap;

pub trait HoconExt: Sized {
    fn as_hash(&self) -> Option<&HashMap<String, Self>>;
    fn as_array(&self) -> Option<&Vec<Self>>;
}

impl HoconExt for Hocon {
    // TODO take owned self here?

    fn as_hash(&self) -> Option<&HashMap<String, Self>> {
        match self {
            Hocon::Hash(x) => Some(x),
            _ => None,
        }
    }
    fn as_array(&self) -> Option<&Vec<Self>> {
        match self {
            Hocon::Array(x) => Some(x),
            _ => None,
        }
    }
}
