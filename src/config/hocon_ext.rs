use anyhow::*;
use hocon::Hocon;
use std::collections::HashMap;

pub trait HoconExt: Sized {
    fn as_hash(&self) -> Result<&HashMap<String, Self>>;
    fn as_array(&self) -> Result<&Vec<Self>>;
}

impl HoconExt for Hocon {
    fn as_hash(&self) -> Result<&HashMap<String, Self>> {
        match self {
            Hocon::Hash(x) => Ok(x),
            _ => Err(anyhow!("as_hash called with {:?}", self)),
        }
    }
    fn as_array(&self) -> Result<&Vec<Self>> {
        match self {
            Hocon::Array(x) => Ok(x),
            _ => Err(anyhow!("as_array called with {:?}", self)),
        }
    }
}
