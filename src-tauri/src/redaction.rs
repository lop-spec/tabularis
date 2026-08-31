use once_cell::sync::Lazy;
use regex::{Captures, Regex};

static CREDENTIAL_URI: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"(?i)\b((?:mysql|mariadb|postgres(?:ql)?|mongodb(?:\+srv)?|redis)://)[^/@\s]+@")
        .expect("credential URI regex")
});
static SECRET_ASSIGNMENT: Lazy<Regex> = Lazy::new(|| {
    Regex::new(
        r#"(?i)\b(password|passwd|pwd|token|secret|authorization)\s*(?:=|:)\s*(?:bearer\s+)?["']?[^\s,;"']+"#,
    )
    .expect("secret assignment regex")
});
static SENSITIVE_METADATA: Lazy<Regex> = Lazy::new(|| {
    Regex::new(
        r#"(?i)\b(database|host|port|account|username|connection(?:\s+name)?)\s*(?:=|:)\s*(?:"[^"]*"|'[^']*'|[^\s,;]+)"#,
    )
    .expect("sensitive metadata regex")
});
static CLOUD_DATABASE_HOST: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"(?i)\b[a-z0-9-]+\.(?:mysql\.)?rds\.[a-z0-9.-]+\b")
        .expect("cloud database host regex")
});
static IPV4: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"\b(?:\d{1,3}\.){3}\d{1,3}\b").expect("IPv4 regex"));

fn public_ipv4(value: &str) -> bool {
    let octets = value
        .split('.')
        .map(str::parse::<u8>)
        .collect::<Result<Vec<_>, _>>();
    let Ok(octets) = octets else {
        return false;
    };
    let [a, b, c, _] = octets.as_slice() else {
        return false;
    };
    !(*a == 0
        || *a == 10
        || *a == 127
        || *a >= 224
        || (*a == 169 && *b == 254)
        || (*a == 172 && (16..=31).contains(b))
        || (*a == 192 && *b == 168)
        || (*a == 192 && *b == 0 && *c == 2)
        || (*a == 198 && *b == 51 && *c == 100)
        || (*a == 203 && *b == 0 && *c == 113))
}

pub fn redact_sensitive_text(value: &str) -> String {
    let value = CREDENTIAL_URI.replace_all(value, "$1<credentials>@");
    let value = SECRET_ASSIGNMENT.replace_all(&value, "$1=<redacted>");
    let value = SENSITIVE_METADATA.replace_all(&value, "$1=<redacted>");
    let value = CLOUD_DATABASE_HOST.replace_all(&value, "<database-host>");
    IPV4.replace_all(&value, |captures: &Captures<'_>| {
        let address = captures
            .get(0)
            .map(|value| value.as_str())
            .unwrap_or_default();
        if public_ipv4(address) {
            "<public-ip>".to_string()
        } else {
            address.to_string()
        }
    })
    .into_owned()
}

fn consume_quoted(chars: &[char], start: usize, quote: char) -> usize {
    let mut index = start + 1;
    while index < chars.len() {
        if chars[index] == '\\' {
            index = (index + 2).min(chars.len());
            continue;
        }
        if chars[index] == quote {
            if index + 1 < chars.len() && chars[index + 1] == quote {
                index += 2;
                continue;
            }
            return index + 1;
        }
        index += 1;
    }
    chars.len()
}

fn consume_dollar_quote(chars: &[char], start: usize) -> Option<usize> {
    if chars.get(start) != Some(&'$') {
        return None;
    }
    let mut tag_end = start + 1;
    while tag_end < chars.len() && (chars[tag_end].is_ascii_alphanumeric() || chars[tag_end] == '_')
    {
        tag_end += 1;
    }
    if chars.get(tag_end) != Some(&'$') {
        return None;
    }
    let tag: String = chars[start..=tag_end].iter().collect();
    let remaining: String = chars[tag_end + 1..].iter().collect();
    remaining
        .find(&tag)
        .map(|offset| tag_end + 1 + offset + tag.chars().count())
}

fn is_identifier_char(value: char) -> bool {
    value.is_ascii_alphanumeric() || value == '_' || value == '$'
}

pub fn redact_sql_literals(sql: &str) -> String {
    let chars: Vec<char> = sql.chars().collect();
    let mut output = String::with_capacity(sql.len());
    let mut index = 0;
    while index < chars.len() {
        let current = chars[index];
        if current == '\'' || current == '"' {
            output.push('?');
            index = consume_quoted(&chars, index, current);
            continue;
        }
        if current == '`' {
            let end = consume_quoted(&chars, index, current);
            output.extend(chars[index..end].iter());
            index = end;
            continue;
        }
        if current == '-' && chars.get(index + 1) == Some(&'-') {
            output.push_str("-- <redacted>");
            index += 2;
            while index < chars.len() && chars[index] != '\n' {
                index += 1;
            }
            continue;
        }
        if current == '#' {
            output.push_str("# <redacted>");
            index += 1;
            while index < chars.len() && chars[index] != '\n' {
                index += 1;
            }
            continue;
        }
        if current == '/' && chars.get(index + 1) == Some(&'*') {
            output.push_str("/* <redacted> */");
            index += 2;
            while index + 1 < chars.len() && !(chars[index] == '*' && chars[index + 1] == '/') {
                index += 1;
            }
            index = (index + 2).min(chars.len());
            continue;
        }
        if current == '$' {
            if let Some(end) = consume_dollar_quote(&chars, index) {
                output.push('?');
                index = end;
                continue;
            }
        }
        let starts_number = current.is_ascii_digit()
            && index
                .checked_sub(1)
                .and_then(|previous| chars.get(previous))
                .map_or(true, |previous| !is_identifier_char(*previous));
        if starts_number {
            output.push('?');
            index += 1;
            while index < chars.len()
                && (chars[index].is_ascii_alphanumeric()
                    || matches!(chars[index], '.' | '+' | '-' | '_'))
            {
                index += 1;
            }
            continue;
        }
        output.push(current);
        index += 1;
    }
    redact_sensitive_text(&output)
}

pub fn redact_log_message(message: &str) -> String {
    let redacted = redact_sensitive_text(message);
    let lowercase = redacted.to_ascii_lowercase();
    let marker = ["query:", "sql:"]
        .into_iter()
        .filter_map(|candidate| lowercase.find(candidate).map(|index| (index, candidate.len())))
        .min_by_key(|(index, _)| *index);
    let Some((index, marker_len)) = marker else {
        return redacted;
    };
    let sql_start = index + marker_len;
    format!(
        "{}{}",
        &redacted[..sql_start],
        redact_sql_literals(&redacted[sql_start..])
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn text_redaction_removes_credentials_and_public_hosts() {
        let public_address = [93, 184, 216, 34].map(|part| part.to_string()).join(".");
        let input = format!(
            "password=hunter2 token: Bearer abc mysql://user:pass@db.example.invalid/app host=db.internal peer {public_address}"
        );
        let output = redact_sensitive_text(&input);
        for secret in [
            "hunter2",
            "abc",
            "user:pass",
            "db.internal",
            public_address.as_str(),
        ] {
            assert!(!output.contains(secret));
        }
        assert!(output.contains("<credentials>"));
        assert!(output.contains("<public-ip>"));
    }

    #[test]
    fn sql_redaction_keeps_shape_and_removes_literals_and_comments() {
        let sql = "UPDATE `users` SET email='alice@example.invalid', quota=4242 WHERE id=7 /* customer 99 */";
        let output = redact_sql_literals(sql);
        assert!(output.starts_with("UPDATE `users` SET email=?"));
        for secret in ["alice@example.invalid", "4242", "customer 99"] {
            assert!(!output.contains(secret));
        }
    }

    #[test]
    fn sql_redaction_handles_escaped_and_dollar_quoted_values() {
        let sql = "SELECT 'it''s private', $tag$private body$tag$, 0xdeadbeef";
        let output = redact_sql_literals(sql);
        assert!(!output.contains("private"));
        assert!(!output.contains("deadbeef"));
        assert_eq!(output.matches('?').count(), 3);
    }

    #[test]
    fn log_redaction_sanitizes_sql_after_a_structured_marker() {
        let output = redact_log_message(
            "Executing on connection: private-id | Query: UPDATE users SET email='private@example.invalid' WHERE id=42",
        );
        for secret in ["private-id", "private@example.invalid", "42"] {
            assert!(!output.contains(secret));
        }
        assert!(output.contains("Query: UPDATE users SET email=?"));
    }
}
