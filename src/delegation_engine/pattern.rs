//! Pattern matching and cloud model detection utilities.

/// Matches strings with simple wildcards (e.g. "gemini*", "*unit test*", "*code*").
pub fn matches_pattern_str(pattern: &str, text: &str) -> bool {
    let p = pattern.trim().to_lowercase();
    let t = text.trim().to_lowercase();
    if p == "*" || p.is_empty() {
        return true;
    }
    if !p.contains('*') {
        return t == p || t.contains(&p);
    }
    let parts: Vec<&str> = p.split('*').collect();
    if parts.len() == 2 {
        let prefix = parts[0];
        let suffix = parts[1];
        if !prefix.is_empty() && !t.starts_with(prefix) {
            return false;
        }
        if !suffix.is_empty() && !t.ends_with(suffix) {
            return false;
        }
        return true;
    }
    let mut current = t.as_str();
    for (i, part) in parts.iter().enumerate() {
        if part.is_empty() {
            continue;
        }
        if i == 0 && !p.starts_with('*') {
            if !current.starts_with(part) {
                return false;
            }
            current = &current[part.len()..];
        } else if i == parts.len() - 1 && !p.ends_with('*') {
            return current.ends_with(part);
        } else {
            match current.find(part) {
                Some(idx) => {
                    current = &current[idx + part.len()..];
                }
                None => return false,
            }
        }
    }
    true
}

/// Detects external cloud model identifiers to prevent accidental token spend.
pub fn is_external_cloud_model(model_name: &str) -> bool {
    let m = model_name.trim().to_lowercase();
    m.starts_with("gemini")
        || m.starts_with("gpt")
        || m.starts_with("claude")
        || m.starts_with("openai")
        || m.starts_with("anthropic")
        || m.starts_with("google")
        || m.contains("flash")
        || m.contains("pro-exp")
        || m.contains("turbo")
}
