use regex::Regex;

// Require a non-alphanumeric boundary so names such as SC0914 do not become C0914.
const LCSC_ID_PATTERN: &str = r"(?i)(?:^|[^A-Z0-9])(C\d{3,})";

pub fn extract_lcsc_ids(content: &str) -> Vec<String> {
    let Ok(re) = Regex::new(LCSC_ID_PATTERN) else {
        return Vec::new();
    };
    let mut ids = Vec::new();
    for captures in re.captures_iter(content) {
        let Some(item) = captures.get(1) else {
            continue;
        };
        let id = item.as_str().to_ascii_uppercase();
        if !ids.iter().any(|existing| existing == &id) {
            ids.push(id);
        }
    }
    ids
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bare_ccode() {
        assert_eq!(extract_lcsc_ids("C2040"), vec!["C2040"]);
    }

    #[test]
    fn extracts_all_lcsc_ids_from_mixed_clipboard_text() {
        let content = "C2040, lcsc.com/product-detail/X_C2927505.html\n编号: c25804 C2040";
        assert_eq!(
            extract_lcsc_ids(content),
            vec!["C2040", "C2927505", "C25804"]
        );
    }

    #[test]
    fn copied_component_metadata_does_not_duplicate_id() {
        let content = "名称：SC0914(13) 型号：RP2040 品牌：Raspberry Pi(树莓派) 封装：LQFN-56(7x7) 编号：C2040";
        assert_eq!(extract_lcsc_ids(content), vec!["C2040"]);
    }
}
