use regex::escape;

use crate::AppError;

pub(crate) fn rule(raw: &str, text: &str, min_chars: u32) -> Result<String, AppError> {
    let leak = raw.trim();
    let min = usize::try_from(min_chars).unwrap_or(usize::MAX).max(1);
    if leak.chars().count() >= min && text.contains(leak) {
        Ok(escape(leak))
    } else {
        Err(AppError::Unprocessable)
    }
}
