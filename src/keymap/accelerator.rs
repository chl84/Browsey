fn normalize_key_token(token: &str) -> Result<String, String> {
    let lowered = token.trim().to_ascii_lowercase();
    if lowered.is_empty() {
        return Err("missing key".into());
    }

    let canonical = match lowered.as_str() {
        "esc" | "escape" => "Escape".to_string(),
        "enter" | "return" => "Enter".to_string(),
        "tab" => "Tab".to_string(),
        "space" | "spacebar" => "Space".to_string(),
        "backspace" => "Backspace".to_string(),
        "delete" | "del" => "Delete".to_string(),
        "insert" | "ins" => "Insert".to_string(),
        "home" => "Home".to_string(),
        "end" => "End".to_string(),
        "pageup" | "pgup" => "PageUp".to_string(),
        "pagedown" | "pgdn" => "PageDown".to_string(),
        "arrowup" | "up" => "ArrowUp".to_string(),
        "arrowdown" | "down" => "ArrowDown".to_string(),
        "arrowleft" | "left" => "ArrowLeft".to_string(),
        "arrowright" | "right" => "ArrowRight".to_string(),
        _ => {
            if lowered.len() == 1 {
                let ch = lowered.chars().next().unwrap_or_default();
                if ch.is_ascii_alphanumeric() {
                    ch.to_ascii_uppercase().to_string()
                } else {
                    return Err(format!("unsupported key '{token}'"));
                }
            } else if lowered.starts_with('f') {
                let number = lowered[1..]
                    .parse::<u8>()
                    .map_err(|_| format!("unsupported key '{token}'"))?;
                if !(1..=24).contains(&number) {
                    return Err(format!("unsupported function key '{token}'"));
                }
                format!("F{number}")
            } else {
                return Err(format!("unsupported key '{token}'"));
            }
        }
    };

    Ok(canonical)
}

pub fn canonicalize_accelerator(raw: &str) -> Result<String, String> {
    let mut ctrl = false;
    let mut alt = false;
    let mut shift = false;
    let mut key: Option<String> = None;

    let mut part_count = 0usize;
    for part in raw.split('+') {
        let token = part.trim();
        if token.is_empty() {
            return Err("invalid shortcut format".into());
        }
        part_count += 1;
        let lowered = token.to_ascii_lowercase();
        match lowered.as_str() {
            "ctrl" | "control" | "cmd" | "command" | "meta" => ctrl = true,
            "alt" | "option" => alt = true,
            "shift" => shift = true,
            _ => {
                if key.is_some() {
                    return Err("shortcut may only include one non-modifier key".into());
                }
                key = Some(normalize_key_token(token)?);
            }
        }
    }

    if part_count == 0 {
        return Err("shortcut cannot be empty".into());
    }

    let key = key.ok_or_else(|| "shortcut must include a key".to_string())?;
    let is_alnum_single = key.len() == 1
        && key
            .chars()
            .next()
            .map(|ch| ch.is_ascii_alphanumeric())
            .unwrap_or(false);
    if is_alnum_single && !ctrl && !alt {
        return Err("alphanumeric shortcuts require Ctrl/Cmd or Alt".into());
    }
    if ctrl && shift && !alt && key == "I" {
        return Err("Ctrl+Shift+I is reserved".into());
    }

    let mut parts: Vec<&str> = Vec::with_capacity(4);
    if ctrl {
        parts.push("Ctrl");
    }
    if alt {
        parts.push("Alt");
    }
    if shift {
        parts.push("Shift");
    }

    let mut out = parts.join("+");
    if !out.is_empty() {
        out.push('+');
    }
    out.push_str(&key);
    Ok(out)
}
