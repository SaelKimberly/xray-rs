use std::borrow::Cow;

use nom::Offset;

mod extra;
mod lines;

pub fn normalize_subscription_content<'a>(content: &'a str) -> Cow<'a, str> {
    let Some(begin) = content
        .lines()
        .map(str::trim_start)
        .find(|l| !(l.starts_with('#') || l.is_empty()))
    else {
        return Cow::Borrowed("");
    };

    let content = &content[content.offset(begin)..];
    let (done, fail, content) = extra::normalize_extras(content);
    if done > 0 {
        tracing::debug!("Normalized {} extra(s)", done);
    }
    if fail > 0 {
        tracing::info!("Failed to normalize {} extra(s)", fail);
    }

    let (done, fail, content) = lines::normalize_lines(content.as_ref());
    if done > 0 {
        tracing::debug!("Normalized {} lines", done);
    }
    if fail > 0 {
        tracing::info!("Failed to normalize {} lines", fail);
    }

    Cow::Owned(content)
}
