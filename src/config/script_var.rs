use anyhow::*;

use crate::ensure_xml_tag_is;

use super::*;

#[derive(Clone, Debug, PartialEq)]
pub struct PollScriptVar {
    pub name: VarName,
    pub command: String,
    pub interval: std::time::Duration,
}

#[derive(Clone, Debug, PartialEq)]
pub struct TailScriptVar {
    pub name: VarName,
    pub command: String,
}

#[derive(Clone, Debug, PartialEq)]
pub enum ScriptVar {
    Poll(PollScriptVar),
    Tail(TailScriptVar),
}

impl ScriptVar {
    pub fn name(&self) -> &VarName {
        match self {
            ScriptVar::Poll(x) => &x.name,
            ScriptVar::Tail(x) => &x.name,
        }
    }

    pub fn initial_value(&self) -> Result<PrimitiveValue> {
        match self {
            ScriptVar::Poll(x) => Ok(crate::run_command(&x.command)?),
            ScriptVar::Tail(_) => Ok(PrimitiveValue::from_string(String::new())),
        }
    }

    pub fn from_xml_element(xml: XmlElement) -> Result<Self> {
        ensure_xml_tag_is!(xml, "script-var");

        let name = VarName(xml.attr("name")?.to_owned());
        let command = xml.only_child()?.as_text()?.text();
        if let Ok(interval) = xml.attr("interval") {
            let interval = util::parse_duration(interval)?;
            Ok(ScriptVar::Poll(PollScriptVar { name, command, interval }))
        } else {
            Ok(ScriptVar::Tail(TailScriptVar { name, command }))
        }
    }
}
