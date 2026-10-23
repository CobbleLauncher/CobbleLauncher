// SPDX-License-Identifier: MIT
//! Small, explicit redaction for common credential forms in Minecraft logs.
//! This is not a general secret scanner; callers must not treat it as complete.

const REDACTED: &str = "[REDACTED]";
const KEYS: [&str; 3] = ["access_token", "refresh_token", "client_secret"];

/// Redact common bearer headers, OAuth-style key/value fields, and Minecraft
/// `--accessToken` arguments while preserving every input line ending.
pub fn redact_log(input: &str) -> String {
    let mut output = String::with_capacity(input.len());
    for segment in input.split_inclusive('\n') {
        let (line, ending) = if let Some(line) = segment.strip_suffix('\n') {
            if let Some(line) = line.strip_suffix('\r') {
                (line, "\r\n")
            } else {
                (line, "\n")
            }
        } else {
            (segment, "")
        };
        output.push_str(&redact_line(line));
        output.push_str(ending);
    }
    output
}

fn redact_line(line: &str) -> String {
    let lower = line.to_ascii_lowercase();
    let mut ranges = Vec::new();

    let mut search_from = 0;
    while let Some(offset) = lower[search_from..].find("authorization:") {
        let start = search_from + offset + "authorization:".len();
        let mut value = start;
        while line
            .as_bytes()
            .get(value)
            .is_some_and(u8::is_ascii_whitespace)
        {
            value += 1;
        }
        if lower[value..].starts_with("bearer") {
            let after_scheme = value + "bearer".len();
            if line
                .as_bytes()
                .get(after_scheme)
                .is_some_and(u8::is_ascii_whitespace)
            {
                let token = skip_ascii_whitespace(line, after_scheme);
                let end = token_end(line, token, true);
                if token < end {
                    ranges.push((token, end));
                }
            }
        }
        search_from = start;
    }

    for key in KEYS {
        let mut search_from = 0;
        while let Some(offset) = lower[search_from..].find(key) {
            let key_start = search_from + offset;
            let mut value = key_start + key.len();
            if line.as_bytes().get(value) == Some(&b'"') {
                value += 1;
            }
            while line
                .as_bytes()
                .get(value)
                .is_some_and(u8::is_ascii_whitespace)
            {
                value += 1;
            }
            let Some(separator) = line.as_bytes().get(value).copied() else {
                break;
            };
            if separator == b'=' || separator == b':' {
                value += 1;
                while line
                    .as_bytes()
                    .get(value)
                    .is_some_and(u8::is_ascii_whitespace)
                {
                    value += 1;
                }
                let (start, end) = scalar_range(line, value);
                if start < end {
                    ranges.push((start, end));
                }
            }
            search_from = key_start + key.len();
        }
    }

    let mut search_from = 0;
    while let Some(offset) = lower[search_from..].find("--accesstoken") {
        let flag = search_from + offset;
        let mut value = flag + "--accesstoken".len();
        if line.as_bytes().get(value) == Some(&b'=') {
            value += 1;
        } else if line
            .as_bytes()
            .get(value)
            .is_some_and(u8::is_ascii_whitespace)
        {
            value = skip_ascii_whitespace(line, value);
        } else {
            search_from = value;
            continue;
        }
        let (start, end) = scalar_range(line, value);
        if start < end {
            ranges.push((start, end));
        }
        search_from = value.max(flag + 1);
    }

    ranges.sort_unstable();
    let mut merged: Vec<(usize, usize)> = Vec::new();
    for (start, end) in ranges {
        if let Some(last) = merged.last_mut()
            && start <= last.1
        {
            last.1 = last.1.max(end);
        } else {
            merged.push((start, end));
        }
    }
    let mut result = String::with_capacity(line.len());
    let mut cursor = 0;
    for (start, end) in merged {
        result.push_str(&line[cursor..start]);
        result.push_str(REDACTED);
        cursor = end;
    }
    result.push_str(&line[cursor..]);
    result
}

fn skip_ascii_whitespace(line: &str, mut index: usize) -> usize {
    while line
        .as_bytes()
        .get(index)
        .is_some_and(u8::is_ascii_whitespace)
    {
        index += 1;
    }
    index
}

fn scalar_range(line: &str, index: usize) -> (usize, usize) {
    let bytes = line.as_bytes();
    if index >= bytes.len() {
        return (index, index);
    }
    let quote = match bytes[index] {
        b'"' | b'\'' => Some(bytes[index]),
        _ => None,
    };
    let start = index + usize::from(quote.is_some());
    let mut end = start;
    while end < bytes.len() {
        let byte = bytes[end];
        if quote == Some(byte) || (quote.is_none() && is_value_delimiter(byte)) {
            break;
        }
        end += 1;
    }
    (start, end)
}

fn token_end(line: &str, index: usize, header: bool) -> usize {
    let mut end = index;
    for (relative, character) in line[index..].char_indices() {
        if character.is_whitespace() || (header && matches!(character, ',' | '"' | '\'')) {
            return index + relative;
        }
        end = index + relative + character.len_utf8();
    }
    end
}

fn is_value_delimiter(byte: u8) -> bool {
    matches!(
        byte,
        b',' | b'&' | b';' | b'}' | b']' | b'"' | b'\'' | b' ' | b'\t'
    )
}

#[cfg(test)]
mod tests {
    use super::redact_log;

    #[test]
    fn redacts_common_header_json_and_key_value_forms() {
        let input = "Authorization: Bearer abc123\n{\"access_token\":\"json-secret\",\"refresh_token\": \"refresh秘密\"}\nclient_secret=plain-secret&keep=visible";
        let output = redact_log(input);
        assert_eq!(
            output,
            "Authorization: Bearer [REDACTED]\n{\"access_token\":\"[REDACTED]\",\"refresh_token\": \"[REDACTED]\"}\nclient_secret=[REDACTED]&keep=visible"
        );
        assert!(!output.contains("abc123"));
        assert!(!output.contains("json-secret"));
        assert!(!output.contains("refresh秘密"));
        assert!(!output.contains("plain-secret"));
    }

    #[test]
    fn redacts_minecraft_argument_and_preserves_line_endings_and_unicode() {
        let input = "启动 --accessToken token-一\r\n--accessToken=token-two\nend";
        let output = redact_log(input);
        assert_eq!(
            output,
            "启动 --accessToken [REDACTED]\r\n--accessToken=[REDACTED]\nend"
        );
        assert_eq!(output.matches('\n').count(), input.matches('\n').count());
        assert_eq!(
            output.matches("\r\n").count(),
            input.matches("\r\n").count()
        );
    }

    #[test]
    fn leaves_unmatched_text_and_empty_values_alone() {
        let input = "access_token=\nAuthorization: Basic abc\nclient_secretish=keep\n";
        assert_eq!(redact_log(input), input);
        assert_eq!(redact_log(""), "");
    }
}
