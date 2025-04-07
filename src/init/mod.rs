use std::str::from_utf8;

use anyhow::{Context, Ok, Result, bail};
use jsonpath_rust::JsonPath;
use reqwest::redirect;

use crate::{
    Os,
    config::{Conf, Environment, PathEnv},
    interpol::InterpolableString,
    resources::{self, Resource},
};

mod java;

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
}
