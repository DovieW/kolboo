use super::types::{WindowsInsertMethod, WindowsInsertPlan};

pub fn build_insert_plan(
    supports_value_pattern: bool,
    allow_paste: bool,
    allow_typing: bool,
) -> WindowsInsertPlan {
    let method = if supports_value_pattern {
        WindowsInsertMethod::UiaValuePattern
    } else if allow_paste {
        WindowsInsertMethod::Paste
    } else if allow_typing {
        WindowsInsertMethod::Typing
    } else {
        WindowsInsertMethod::None
    };

    let allowed = method != WindowsInsertMethod::None;
    let reason = match method {
        WindowsInsertMethod::UiaValuePattern => "uia_value_pattern".to_string(),
        WindowsInsertMethod::Paste => "paste_fallback".to_string(),
        WindowsInsertMethod::Typing => "typing_fallback".to_string(),
        WindowsInsertMethod::None => "no_available_method".to_string(),
    };

    WindowsInsertPlan {
        method,
        reason,
        allowed,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn chooses_value_pattern_first() {
        let plan = build_insert_plan(true, true, true);
        assert_eq!(plan.method, WindowsInsertMethod::UiaValuePattern);
        assert!(plan.allowed);
    }

    #[test]
    fn falls_back_to_paste() {
        let plan = build_insert_plan(false, true, true);
        assert_eq!(plan.method, WindowsInsertMethod::Paste);
        assert!(plan.allowed);
    }

    #[test]
    fn falls_back_to_typing() {
        let plan = build_insert_plan(false, false, true);
        assert_eq!(plan.method, WindowsInsertMethod::Typing);
        assert!(plan.allowed);
    }

    #[test]
    fn returns_none_when_disallowed() {
        let plan = build_insert_plan(false, false, false);
        assert_eq!(plan.method, WindowsInsertMethod::None);
        assert!(!plan.allowed);
    }
}
