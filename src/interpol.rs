use std::collections::HashMap;

pub struct Env(pub HashMap<String, String>);

impl Env {
    pub fn interpolate(&self, s: &str) -> String {
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
                        Some(val) => result.push_str(val),
                        None => result.push_str(&format!("${{{}}}", key)),
                    }
                }
            } else {
                result.push(c);
            }
        }
        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_interpolate() {
        let env = Env(vec![
            ("FOO".to_string(), "bar".to_string()),
            ("BAZ".to_string(), "qux".to_string()),
        ]
        .into_iter()
        .collect());
        assert_eq!(env.interpolate("hello ${FOO}"), "hello bar");
        assert_eq!(env.interpolate("hello ${FOO} ${BAZ}"), "hello bar qux");
        assert_eq!(
            env.interpolate("hello ${FOO} ${BAZ} ${QUUX}"),
            "hello bar qux ${QUUX}"
        );
    }
}
