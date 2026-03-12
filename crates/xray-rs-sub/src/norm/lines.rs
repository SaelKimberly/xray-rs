use nom::Offset;

static KNOWN_SCHEMAS: &[&str] = &[
    "vless://",
    "vmess://",
    "trojan://",
    "shadowsocks://",
    "ss://",
    "ssr://",
    "http://",
    "https://",
    "socks://",
    "socks5://",
    "hysteria2://",
    "hysteria://",
    "hhysteria2://",
    "hhysteria://",
    "hy2://",
    "hy://",
    "anytls://",
];

pub fn normalize_lines<'a>(s: &'a str) -> (usize, usize, String) {
    // NOTE: this function should be called after JSON parameters normalization    // as URLs cannot contain newlines or whitespace, we can just split
    let mut result = String::new();
    let mut done = 0_usize;
    let mut fail = 0_usize;
    for chunk in s
        .split(char::is_whitespace)
        .flat_map(|s| s.split_inclusive("://"))
    {
        let mut chunk: &str = chunk;
        let mut schema_suffix = Option::<&'a str>::None;
        let mut schema_prefix = Option::<&'a str>::None;
        for schema in KNOWN_SCHEMAS {
            // trim schema at the end
            chunk = if schema_suffix.is_none()
                && let Some(chunk) = chunk.strip_suffix(schema)
            {
                schema_suffix = Some(schema);
                chunk
            } else {
                chunk
            };

            // trim schema at the beginning
            chunk = if schema_prefix.is_none()
                && let Some(chunk) = chunk.strip_prefix(schema)
            {
                schema_prefix = Some(schema);
                chunk
            } else {
                chunk
            };
        }

        let Some(schema_prefix) = schema_prefix else {
            tracing::warn!("Chunk without schema at {}", chunk.offset(s));
            fail += 1;
            continue;
        };
        done += 1;
        result.push_str(schema_prefix);

        if let Some(schema_suffix) = schema_suffix {
            result.push_str(chunk);
            result.push('\n');
            result.push_str(schema_suffix);
        } else {
            if let Some(in_chunk) = chunk.strip_suffix("://") {
                if let Some((unknown_schema_start, _)) = in_chunk
                    .char_indices()
                    .rev()
                    .skip(3)
                    .take_while(|(_, c)| c.is_ascii_alphanumeric())
                    .last()
                {
                    let unknown_schema = &chunk[unknown_schema_start..];
                    chunk = &chunk[..unknown_schema_start];

                    tracing::warn!(
                        "Chunk ended with unknown schema: {} at {}",
                        unknown_schema,
                        unknown_schema.offset(s)
                    );
                } else {
                    let corruption_place = chunk[chunk.len() - 3..].offset(s);
                    chunk = &chunk[..chunk.len() - 3];

                    tracing::warn!(
                        "Chunk ended with corrupted schema: {} at {}",
                        chunk,
                        corruption_place
                    );
                }
            }

            result.push_str(chunk);
            result.push('\n');
        }
    }
    (done, fail, result)
}
