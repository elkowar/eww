use std::{path::PathBuf, process::Command};

use anyhow::*;

use crate::ensure_xml_tag_is;

use super::*;

type Interval = Option<std::time::Duration>;
type Files = Vec<PathBuf>;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum VarSource {
    Shell(String),
    Function(fn() -> Result<PrimVal>),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PollScriptVar {
    pub name: VarName,
    pub command: VarSource,
    pub interval: Interval,
    pub files: Files,
}

impl PollScriptVar {
    pub fn run_once(&self) -> Result<PrimVal> {
        match &self.command {
            VarSource::Shell(x) => run_command(x),
            VarSource::Function(x) => x(),
        }
    }
    pub fn change_delay(&mut self, a: Interval) -> () {
        self.interval = a;
    }

    pub fn change_files(&mut self, a: Files) -> () {
        self.files = a;
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
                    run_command(&f).with_context(|| format!("Failed to compute initial value for {}", &self.name()))
                }
            },
            ScriptVar::Tail(_) => Ok(PrimVal::from_string(String::new())),
        }
    }

    pub fn from_xml_element(xml: XmlElement) -> Result<Self> {
        ensure_xml_tag_is!(xml, "script-var");

        let name = VarName(xml.attr("name")?);
        let command = xml.only_child()?.as_text()?.text();

        let interval = xml.attr("interval");
        let files = xml.attr("files");

        if interval.is_err() && files.is_err() {
            return Ok(ScriptVar::Tail(TailScriptVar { name, command }))
        }

        Ok(ScriptVar::Poll(PollScriptVar {
            name,
            command: VarSource::Shell(command),
            interval: if interval.is_err() { None } else { Some(util::parse_duration(&interval.unwrap())?) },
            // empty vec doesn't allocate on heap, so no considerable larger memory footprint
            files: if files.is_err() { vec![] } else { PrimVal::from_string(files.unwrap()).as_vec()?.iter().map(PathBuf::from).collect() }
        }))

    }
}

/// Run a command and get the output
fn run_command(cmd: &str) -> Result<PrimVal> {
    log::debug!("Running command: {}", cmd);
    let output = String::from_utf8(Command::new("/bin/sh").arg("-c").arg(cmd).output()?.stdout)?;
    let output = output.trim_matches('\n');
    Ok(PrimVal::from(output))
}
