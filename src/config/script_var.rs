use std::process::Command;

use anyhow::*;

use crate::ensure_xml_tag_is;

use super::*;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PollScriptVar {
    pub name: VarName,
    pub command: String,
    pub interval: std::time::Duration,
}

impl PollScriptVar {
    pub fn run_once(&self) -> Result<PrimitiveValue> {
        run_command(&self.command)
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TailScriptVar {
    pub name: VarName,
    pub command: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
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
            ScriptVar::Poll(x) => Ok(run_command(&x.command)?),
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

/// Run a command and get the output
fn run_command(cmd: &str) -> Result<PrimitiveValue> {
    let output = String::from_utf8(Command::new("/bin/sh").arg("-c").arg(cmd).output()?.stdout)?;
    let output = output.trim_matches('\n');
    Ok(PrimitiveValue::from(output))
}
