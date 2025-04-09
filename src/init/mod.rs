use anyhow::{Ok, Result};

use crate::config::Conf;

mod golang;
mod java;
mod node;
mod python;

#[derive(Debug, Clone, Copy)]
pub(crate) enum JavaBuildTool {
    Sbt,
    Maven,
}

impl Conf {
    pub(crate) fn init_java(version: u8, jbt_opt: &Option<JavaBuildTool>) -> Result<Conf> {
        let java = java::java(version)?;
        if let Some(jbt) = jbt_opt {
            let jbt_res = match jbt {
                JavaBuildTool::Sbt => java::sbt()?,
                JavaBuildTool::Maven => java::maven()?,
            };
            Ok(Conf {
                shell: Some(java.merge(jbt_res)?),
                builder: None,
            })
        } else {
            Ok(Conf {
                shell: Some(java),
                builder: None,
            })
        }
    }

    pub(crate) fn init_go() -> Result<Conf> {
        let go = golang::go()?;
        Ok(Conf {
            shell: Some(go),
            builder: None,
        })
    }

    pub(crate) fn init_node() -> Result<Conf> {
        let node = node::node()?;
        Ok(Conf {
            shell: Some(node),
            builder: None,
        })
    }

    pub(crate) fn init_python() -> Result<Conf> {
        let python = python::python()?;
        Ok(Conf {
            shell: Some(python),
            builder: None,
        })
    }
}
