use std::collections::HashMap;

use anyhow::{Result, anyhow};

use crate::resources::Substrate;

pub struct Env(pub HashMap<String, Substrate>);

impl Env {
    pub fn interpolate(&self, s: &str) -> Result<String> {
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
                    let key = &s[start + 1..end];
                    match self.0.get(key) {
                        Some(val) => result.push_str(&val.to_string()),
                        None => return Err(anyhow!("missing key: {}", key)),
                    }
                }
            } else {
                result.push(c);
            }
        }
        Ok(result)
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
        assert_eq!(env.interpolate("hello ${FOO}")?, "hello bar");
        assert_eq!(env.interpolate("hello ${FOO} ${BAZ}")?, "hello bar qux");
        assert!(env.interpolate("hello ${FOO} ${BAZ} ${QUUX}").is_err());
        Ok(())
    }
}
