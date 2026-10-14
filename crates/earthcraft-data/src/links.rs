//! External geographic resource URLs. Opening them is a frontend responsibility.
pub fn photo_search(name: &str) -> String {
    // Wikimedia Commons Special:MediaSearch; human-selected photographs, not invented imagery.
    format!(
        "https://commons.wikimedia.org/wiki/Special:MediaSearch?type=image&search={}",
        encode(name)
    )
}
fn encode(s: &str) -> String {
    let mut out = String::new();
    for byte in s.bytes() {
        if byte.is_ascii_alphanumeric() || b"-_.~".contains(&byte) {
            out.push(byte as char)
        } else {
            out.push_str(&format!("%{byte:02X}"));
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn photo_query_encodes_utf8_and_reserved_characters() {
        assert_eq!(
            photo_search("São Paulo & Rio"),
            "https://commons.wikimedia.org/wiki/Special:MediaSearch?type=image&search=S%C3%A3o%20Paulo%20%26%20Rio"
        );
    }
}
