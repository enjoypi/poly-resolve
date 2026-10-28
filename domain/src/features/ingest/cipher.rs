pub(super) fn fragments(text: &str, min_length: usize) -> bool {
    text.lines().any(|line| {
        let trimmed = line.trim();
        trimmed.len() >= min_length.max(1) && strict_base64(trimmed)
    })
}

fn numeric(part: &str) -> bool {
    !part.is_empty() && part.bytes().all(|byte| byte.is_ascii_digit())
}

fn plain_base64(body: &str) -> bool {
    body.bytes().all(|byte| {
        byte.is_ascii_alphanumeric()
            || matches!(byte, b'+' | b'/' | b'=' | b'\n' | b'\r' | b' ' | b'\t')
    })
}

pub(in crate::features) fn sealed(text: &str, min_length: usize) -> bool {
    let parts: Vec<&str> = text.trim().rsplitn(4, "||").collect();
    let Ok([length, kind, version, raw]) = <[&str; 4]>::try_from(parts) else {
        return false;
    };
    let body = raw.trim();
    numeric(length)
        && numeric(kind)
        && numeric(version)
        && body.len() >= min_length
        && plain_base64(body)
}

fn strict_base64(line: &str) -> bool {
    line.bytes()
        .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'+' | b'/' | b'='))
}
