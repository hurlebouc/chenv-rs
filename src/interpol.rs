use std::collections::HashMap;

use anyhow::{Result, anyhow};
use serde::{Deserialize, Serialize};

use crate::resources::Substrate;

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct InterpolableString(String);
impl InterpolableString {
    pub fn new(s: String) -> InterpolableString {
        InterpolableString(s)
    }

    pub fn interpolate(&self, env: &Env) -> Result<String> {
        env.interpolate_str(&self.0)
    }

    pub fn get_variables(&self) -> Vec<&str> {
        // TODO réfléchir à comment factoriser ce code avec la fonction interpolate_str
        let mut result = Vec::new();
        let mut chars = self.0.chars().enumerate();
        while let Some((_, c)) = chars.next() {
            if c == '$' {
                if let Some((start, '{')) = chars.next() {
                    let mut end = 0;
                    while let Some((e, c)) = chars.next() {
                        if c == '}' {
                            end = e;
                            break;
                        }
                    }
                    let expr = &self.0[start + 1..end];
                    match expr.split_once('.') {
                        Some((var, _)) => {
                            result.push(var);
                        }
                        None => {
                            result.push(expr);
                        }
                    }
                }
            }
        }
        result
    }
}

pub struct Env(pub HashMap<String, Substrate>);

impl Env {
    fn interpolate_str(&self, s: &str) -> Result<String> {
        let mut result = String::new();
        let mut chars = s.chars().enumerate();
        while let Some((_, c)) = chars.next() {
            if c == '$' {
                if let Some((start, '{')) = chars.next() {
                    let mut end = 0;
                    while let Some((e, c)) = chars.next() {
                        if c == '}' {
                            end = e;
                            break;
                        }
                    }
                    let expr = &s[start + 1..end];
                    let (key, jp) = expr
                        .split_once('.')
                        .map(|(key, jp)| (key, Some(format!("$.{}", jp))))
                        .unwrap_or((expr, None));

                    let val = match self.0.get(key) {
                        Some(val) => match jp {
                            Some(jp) => val.resolve(&jp)?,
                            None => val.to_string(),
                        },
                        None => return Err(anyhow!("missing key: {}", expr)),
                    };

                    result.push_str(&val);
                }
            } else {
                result.push(c);
            }
        }
        Ok(result)
    }

    pub fn interpolate(&self, s: &InterpolableString) -> Result<String> {
        self.interpolate_str(&s.0)
    }

    pub fn new() -> Self {
        Self(HashMap::new())
    }

    pub fn insert(&mut self, key: String, value: Substrate) {
        self.0.insert(key, value);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use anyhow::Result;
    #[test]
    fn test_interpolate() -> Result<()> {
        let env = Env(vec![
            ("FOO".to_string(), Substrate::new("bar".to_string())),
            ("BAZ".to_string(), Substrate::new("qux".to_string())),
        ]
        .into_iter()
        .collect());
        assert_eq!(env.interpolate_str("hello ${FOO}")?, "hello bar");
        assert_eq!(env.interpolate_str("hello ${FOO} ${BAZ}")?, "hello bar qux");
        assert!(env.interpolate_str("hello ${FOO} ${BAZ} ${QUUX}").is_err());
        Ok(())
    }
}
