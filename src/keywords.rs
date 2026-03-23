use regex::Regex;
use std::collections::HashSet;
use std::sync::OnceLock;

const FILTERED_WORDS: &[&str] = &["http", "https", "www"];

fn url_regex() -> &'static Regex {
    static URL_REGEX: OnceLock<Regex> = OnceLock::new();
    URL_REGEX.get_or_init(|| Regex::new(r"https?://([^/\s]+)").expect("valid URL regex"))
}

fn path_regex() -> &'static Regex {
    static PATH_REGEX: OnceLock<Regex> = OnceLock::new();
    PATH_REGEX.get_or_init(|| Regex::new(r"https?://[^/\s]+/([^\s?]+)").expect("valid path regex"))
}

fn header_regex() -> &'static Regex {
    static HEADER_REGEX: OnceLock<Regex> = OnceLock::new();
    HEADER_REGEX.get_or_init(|| Regex::new(r#"-H\s+["']([^"']+)["']"#).expect("valid header regex"))
}

fn method_regex() -> &'static Regex {
    static METHOD_REGEX: OnceLock<Regex> = OnceLock::new();
    METHOD_REGEX.get_or_init(|| Regex::new(r"-X\s+(\w+)").expect("valid method regex"))
}

fn word_regex() -> &'static Regex {
    static WORD_REGEX: OnceLock<Regex> = OnceLock::new();
    WORD_REGEX
        .get_or_init(|| Regex::new(r"\b[a-zA-Z][a-zA-Z0-9_-]{2,}\b").expect("valid word regex"))
}

pub(crate) fn extract_keywords(command: &str) -> Vec<String> {
    let mut keywords = HashSet::new();

    for cap in url_regex().captures_iter(command) {
        if let Some(domain) = cap.get(1) {
            let domain_str = domain.as_str().to_lowercase();
            keywords.insert(domain_str.clone());

            for part in domain_str.split('.') {
                if !part.is_empty() && part.len() > 2 && part != "www" {
                    keywords.insert(part.to_string());
                }
            }
        }
    }

    for cap in path_regex().captures_iter(command) {
        if let Some(path) = cap.get(1) {
            for segment in path.as_str().split('/') {
                if !segment.is_empty() && segment.len() > 2 {
                    keywords.insert(segment.to_lowercase());
                }
            }
        }
    }

    for cap in header_regex().captures_iter(command) {
        if let Some(header) = cap.get(1) {
            let header_str = header.as_str();
            if let Some((header_name, header_value)) = header_str.split_once(':') {
                let header_name = header_name.trim().to_lowercase();
                if !header_name.is_empty() {
                    keywords.insert(header_name);
                }

                for word in header_value.split_whitespace() {
                    if word.len() > 2 {
                        keywords.insert(word.to_lowercase());
                    }
                }
            }
        }
    }

    for cap in method_regex().captures_iter(command) {
        if let Some(method) = cap.get(1) {
            keywords.insert(method.as_str().to_lowercase());
        }
    }

    for cap in word_regex().find_iter(command) {
        let word = cap.as_str().to_lowercase();
        if !FILTERED_WORDS.contains(&word.as_str()) {
            keywords.insert(word);
        }
    }

    let mut keywords: Vec<String> = keywords.into_iter().collect();
    keywords.sort();
    keywords
}

#[cfg(test)]
mod tests {
    use super::extract_keywords;

    #[test]
    fn test_extract_keywords_for_http_command() {
        let command = "curl -X POST https://api.github.com/user/repos -H 'Authorization: token xyz' -d '{\"name\":\"test\"}'";
        let keywords = extract_keywords(command);

        assert!(keywords.contains(&"github".to_string()));
        assert!(keywords.contains(&"api".to_string()));
        assert!(keywords.contains(&"user".to_string()));
        assert!(keywords.contains(&"repos".to_string()));
        assert!(keywords.contains(&"authorization".to_string()));
        assert!(keywords.contains(&"post".to_string()));
        assert!(keywords.contains(&"token".to_string()));
        assert!(keywords.contains(&"name".to_string()));
        assert!(keywords.contains(&"test".to_string()));
    }

    #[test]
    fn test_extract_keywords_with_domain_parts() {
        let command = "curl https://subdomain.example.com/api/v1/data";
        let keywords = extract_keywords(command);

        assert!(keywords.contains(&"subdomain.example.com".to_string()));
        assert!(keywords.contains(&"subdomain".to_string()));
        assert!(keywords.contains(&"example".to_string()));
        assert!(keywords.contains(&"com".to_string()));
        assert!(keywords.contains(&"api".to_string()));
        assert!(keywords.contains(&"data".to_string()));
    }

    #[test]
    fn test_extract_keywords_with_headers() {
        let command = r#"curl -H "Content-Type: application/json" -H "Authorization: Bearer xyz" https://api.example.com"#;
        let keywords = extract_keywords(command);

        assert!(keywords.contains(&"content-type".to_string()));
        assert!(keywords.contains(&"authorization".to_string()));
        assert!(keywords.contains(&"application".to_string()));
        assert!(keywords.contains(&"bearer".to_string()));
        assert!(keywords.contains(&"example".to_string()));
        assert!(keywords.contains(&"api".to_string()));
    }

    #[test]
    fn test_extract_keywords_filters_common_protocol_words() {
        let command = "curl https://www.example.com/api";
        let keywords = extract_keywords(command);

        assert!(keywords.contains(&"curl".to_string()));
        assert!(keywords.contains(&"example".to_string()));
        assert!(keywords.contains(&"api".to_string()));
        assert!(!keywords.contains(&"http".to_string()));
        assert!(!keywords.contains(&"https".to_string()));
        assert!(!keywords.contains(&"www".to_string()));
    }

    #[test]
    fn test_extract_keywords_for_non_http_command() {
        let command = "kubectl rollout restart deployment api-server --namespace platform";
        let keywords = extract_keywords(command);

        assert!(keywords.contains(&"kubectl".to_string()));
        assert!(keywords.contains(&"rollout".to_string()));
        assert!(keywords.contains(&"restart".to_string()));
        assert!(keywords.contains(&"deployment".to_string()));
        assert!(keywords.contains(&"api-server".to_string()));
        assert!(keywords.contains(&"platform".to_string()));
    }
}
