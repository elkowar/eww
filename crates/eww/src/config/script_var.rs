use std::process::Command;

use anyhow::*;

use crate::ensure_xml_tag_is;

use super::*;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum VarSource {
    Shell(String),
    Function(fn() -> Result<PrimVal>),
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PollScriptVar {
    pub name: VarName,
    pub command: VarSource,
    pub interval: std::time::Duration,
}

impl PollScriptVar {
    pub fn run_once(&self) -> Result<PrimVal> {
        match &self.command {
            VarSource::Shell(x) => run_command(x),
            VarSource::Function(x) => x(),
        }
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

    pub fn initial_value(&self) -> Result<PrimVal> {
        match self {
            ScriptVar::Poll(x) => match &x.command {
                VarSource::Function(f) => f().with_context(|| format!("Failed to compute initial value for {}", &self.name())),
                VarSource::Shell(f) => {
                    run_command(f).with_context(|| format!("Failed to compute initial value for {}", &self.name()))
                }
            },
            ScriptVar::Tail(_) => Ok(PrimVal::from_string(String::new())),
        }
    }

    pub fn from_xml_element(xml: XmlElement) -> Result<Self> {
        ensure_xml_tag_is!(xml, "script-var");

        let name = VarName(xml.attr("name")?);
        let command = xml.only_child()?.as_text()?.text();
        if let Ok(interval) = xml.attr("interval") {
            let interval = util::parse_duration(&interval)?;
            Ok(ScriptVar::Poll(PollScriptVar { name, command: crate::config::VarSource::Shell(command), interval }))
        } else {
            Ok(ScriptVar::Tail(TailScriptVar { name, command }))
        }
    }
}

/// Run a command and get the output
fn run_command(cmd: &str) -> Result<PrimVal> {
    log::debug!("Running command: {}", cmd);
    let output = String::from_utf8(Command::new("/bin/sh").arg("-c").arg(cmd).output()?.stdout)?;
    let output = output.trim_matches('\n');
    Ok(PrimVal::from(output))
}
