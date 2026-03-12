use std::{borrow::Cow, ops::Range};

use logos::Logos;
use serde_json::{Map, Value};

mod xlex {
    use logos::Lexer;
    use serde_json::Value;

    use super::ExtraParam;

    const TRIM_PATTERN: [char; 7] = ['+', ' ', '\t', '\n', '\r', '"', '\''];

    #[inline]
    fn to_key_value_pair(s: &str) -> (&str, &str) {
        let Some((l_part, r_part)) = s.rsplit_once(':').or_else(|| s.rsplit_once('=')) else {
            unreachable!("Should never be called with strings without '=' or ':' characters");
        };
        (
            l_part.trim_matches(TRIM_PATTERN),
            r_part.trim_matches(TRIM_PATTERN),
        )
    }

    #[inline]
    fn to_number(s: &str, err: &'static str) -> u64 {
        s.strip_suffix(".0").unwrap_or(s).parse::<u64>().expect(err)
    }

    pub(super) fn _get_key(lex: &mut Lexer<ExtraParam>) -> String {
        const ERR: &str = "Invalid key lexer";

        let (name, value_raw) = to_key_value_pair(lex.slice());
        value_raw
            .is_empty()
            .then_some(name)
            .map(ToOwned::to_owned)
            .expect(ERR)
    }

    pub(super) fn _get_value_const(lex: &mut Lexer<ExtraParam>) -> (String, Value) {
        const ERR: &str = "Invalid value_const lexer";

        let (name, value_raw) = to_key_value_pair(lex.slice());
        (
            name.to_owned(),
            Value::Number(to_number(value_raw, ERR).into()),
        )
    }

    pub(super) fn _get_value_range(lex: &mut Lexer<ExtraParam>) -> (String, Value) {
        const ERR: &str = "Invalid value_range lexer";

        let (name, value_raw) = to_key_value_pair(lex.slice());
        let (l_part, r_part) = value_raw.split_once('-').expect(ERR);
        let (l_part, r_part) = (to_number(l_part, ERR), to_number(r_part, ERR));
        let value = if l_part < r_part {
            format!("{l_part}-{r_part}")
        } else {
            format!("{r_part}-{l_part}")
        };
        (name.to_owned(), Value::String(value))
    }

    pub(super) fn _get_value_text(lex: &mut Lexer<ExtraParam>) -> (String, Value) {
        let (key, value) = to_key_value_pair(lex.slice());
        (key.to_owned(), Value::String(value.into()))
    }

    pub(super) fn _get_value_bool(lex: &mut Lexer<ExtraParam>) -> (String, Value) {
        let (key, value) = to_key_value_pair(lex.slice());
        let value = match value.to_lowercase().as_str() {
            "false" => Value::Bool(false),
            "true" => Value::Bool(true),
            "null" | "none" => Value::Null,
            _ => panic!("Invalid get_value_bool lexer"),
        };
        (key.to_owned(), value)
    }
}

#[derive(Logos, Clone, Debug)]
#[logos(skip "[[:space:]]+")]
#[logos(skip "[+]")]
#[logos(skip "[.]0")]
#[logos(error = String)]
#[logos(subpattern bool_var = r#"([Ff]alse|[Tt]rue|null|None)"#)]
#[logos(subpattern text_var = r#"('[^']+'|"[^"]+")"#)]
#[logos(subpattern usize_var = r#"(0|[1-9][0-9]*([.]0)?)"#)]
#[logos(subpattern range_var = r#"(?&usize_var)-(?&usize_var)"#)]
#[logos(subpattern param = r#"([A-Za-z_-]+|(?&text_var))"#)]
#[logos(subpattern key = r#"(?&param)[\s\+]*[:=][\s\+]*"#)]
#[logos(subpattern value_range = r#"(?&key)(?&range_var)"#)]
#[logos(subpattern value_const = r#"(?&key)(?&usize_var)"#)]
#[logos(subpattern value_bool = r#"(?&key)(?&bool_var)"#)]
#[logos(subpattern value_text = r#"(?&key)(?&param)"#)]
enum ExtraParam {
    #[token("}")]
    Close,
    #[token("{")]
    Begin,
    #[token(",")]
    Comma,
    #[regex(r#"(?&value_const)"#, xlex::_get_value_const)]
    #[regex(r#"(?&value_range)"#, xlex::_get_value_range)]
    #[regex(r#"(?&value_bool)"#, xlex::_get_value_bool)]
    #[regex(r#"(?&value_text)"#, xlex::_get_value_text)]
    Tuple((String, Value)),

    #[regex(r#"(?&key)"#, xlex::_get_key)]
    Param(String),
}

fn try_normalize_not_encoded(s: &str) -> Option<(Range<usize>, Value)> {
    let mut mapping: Map<String, Value> = Map::new();

    let mut path = vec![];
    let mut counter = 0_usize;

    let mut lex = ExtraParam::lexer(s);
    let mut begin = 0;
    let mut close = 0;
    let mut comma: bool = false;

    while let Some(t) = lex.next() {
        let mut cursor: &mut Map<String, Value> = &mut mapping;

        for p in path.iter() {
            let value = cursor.entry(p).or_insert(Value::Object(Default::default()));
            cursor = value.as_object_mut()?;
        }

        let Ok(t) = t else {
            return None;
        };

        match t {
            ExtraParam::Comma if comma => {
                comma = false;
            }
            ExtraParam::Comma => {
                return None;
            }
            ExtraParam::Begin => {
                if counter == 0 {
                    begin = lex.span().start;
                }

                counter += 1;
            }
            ExtraParam::Param(p) => {
                path.push(p);
            }
            ExtraParam::Close => {
                _ = path.pop();

                counter -= 1;
                if counter == 0 {
                    close = lex.span().end;
                    break;
                } else {
                    comma = true;
                }
            }
            ExtraParam::Tuple((n, v)) => {
                match v {
                    Value::String(ref s) if s.is_empty() => {
                        tracing::debug!("Skip {n} key due to empty text value");
                    }
                    Value::Null => {
                        tracing::debug!("Skip {n} key due to null value");
                    }
                    _ => {
                        tracing::debug!("Push {n} key with value {v:?}");
                        cursor.insert(n, v);
                    }
                }
                comma = true;
            }
        }
    }
    if counter > 0 {
        tracing::warn!("Extra not closed");
        return None;
    }
    Some((begin..close, Value::Object(mapping)))
}

fn try_normalize_url_encoded(s: &str) -> Option<(Range<usize>, Value)> {
    if let Some((i, _)) = s
        .char_indices()
        .take_while(|(_, c)| !c.is_whitespace() && !matches!(c, '&' | '#'))
        .last()
    {
        let (s, _) = s.split_at(i + 1);
        let slice = urlencoding::decode(s)
            .inspect_err(|e| tracing::debug!("Invalid extra found: {} ({})", s, e))
            .ok()?;
        if let Some((_, v)) = try_normalize_not_encoded(slice.as_ref()) {
            Some((0..i + 1, v))
        } else {
            tracing::debug!("Invalid extra found: {}", s);
            None
        }
    } else {
        tracing::debug!("Empty extra found");
        None
    }
}

#[allow(unreachable_code, unused_mut)]
pub fn normalize_extras(s: &str) -> (u32, u32, Cow<'_, str>) {
    let mut result: String = String::with_capacity(s.len());

    let mut success_cnt: u32 = 0;
    let mut broken_cnt: u32 = 0;

    let mut last_end = 0_usize;
    for (i, _) in s.match_indices("extra=") {
        result.push_str(&s[last_end..i + 6]);

        let slice = s[i + 6..].trim_start();

        if slice.starts_with("null") || slice.starts_with(['#', '&']) {
            tracing::debug!("Skipping empty extra");
            last_end = i + 6;
            continue;
        }

        let Some((r, mut v, not_encoded)) = (if slice.starts_with('%') {
            try_normalize_url_encoded(slice).map(|(r, v)| (r, v, false))
        } else {
            try_normalize_not_encoded(slice).map(|(r, v)| (r, v, true))
        }) else {
            tracing::warn!("<extra> failed to normalize at byte offset {}", i);
            last_end = i + 6;
            broken_cnt += 1;
            continue;
        };

        let old_json = &slice[r.start..r.end];

        if let Some(Value::Object(m)) = v.get("headers")
            && m.is_empty()
        {
            _ = v.as_object_mut().unwrap().remove("headers");
        }

        let new_json = serde_json::to_string(&v).unwrap();

        if old_json != new_json {
            if not_encoded {
                tracing::debug!("<extra> (n/e): {} -> {}", old_json, new_json);
            } else {
                tracing::debug!("<extra> (url): {} -> {}", old_json, new_json);
            }
        } else if not_encoded {
            tracing::debug!("<extra> (n/e): {}", old_json);
        } else {
            tracing::debug!("<extra> (url): {}", old_json);
        }

        success_cnt += 1;
        result.push_str(urlencoding::encode(new_json.as_str()).as_ref());

        last_end = i + 6 + r.end;
    }

    if last_end > 0 {
        result.push_str(&s[last_end..]);
        (success_cnt, broken_cnt, Cow::Owned(result))
    } else {
        (success_cnt, broken_cnt, Cow::Borrowed(s))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn test_norm_extra_simple_1() {
        let s = "extra={'a': 'b', 'c': 'd', 'e': {'f': 12}}";
        let (done, fail, s_new) = normalize_extras(s);

        assert_eq!(done, 1);
        assert_eq!(fail, 0);

        let s_new = urlencoding::decode(s_new.strip_prefix("extra=").unwrap()).unwrap();
        assert_eq!(s_new, r#"{"a":"b","c":"d","e":{"f":12}}"#);

        let u = "extra=%7B%27a%27%3A%20%27b%27%2C%20%27c%27%3A%20%27d%27%2C%20%27e%27%3A%20%7B%27f%27%3A%2012%7D%7D";
        let (done, fail, u_new) = normalize_extras(u);

        assert_eq!(done, 1);
        assert_eq!(fail, 0);

        let u_new = urlencoding::decode(u_new.strip_prefix("extra=").unwrap()).unwrap();
        assert_eq!(u_new, s_new);

        let s = "extra={'a': 'b', 'c': 'd', 'e': {'f':";
        let (done, fail, s_new) = normalize_extras(s);

        assert_eq!(done, 0);
        assert_eq!(fail, 1);
        assert_eq!(s, s_new);
    }
}
