use crate::Result;
use regex::Regex;
use std::collections::HashSet;
use std::fs;
use std::path::PathBuf;
use std::sync::OnceLock;

fn history_curl_regex() -> &'static Regex {
    static HISTORY_CURL_REGEX: OnceLock<Regex> = OnceLock::new();
    HISTORY_CURL_REGEX.get_or_init(|| Regex::new(r"^(\s*curl\s+.*)$").expect("valid history regex"))
}

pub(crate) fn parse_curl_commands_from_history(history_content: &str) -> Vec<String> {
    let mut curl_commands = Vec::new();
    let mut seen = HashSet::new();

    for line in history_content.lines() {
        let clean_line = if line.starts_with(": ") {
            if let Some(semicolon_pos) = line.find(';') {
                &line[semicolon_pos + 1..]
            } else {
                line
            }
        } else {
            line
        };

        if let Some(cap) = history_curl_regex().captures(clean_line) {
            if let Some(curl_cmd) = cap.get(1) {
                let cmd = curl_cmd.as_str().trim().to_string();
                if seen.insert(cmd.clone()) {
                    curl_commands.push(cmd);
                }
            }
        }
    }

    curl_commands
}

pub(crate) fn parse_curl_commands_from_history_bytes(history_content: &[u8]) -> Vec<String> {
    let history_content = String::from_utf8_lossy(history_content);
    parse_curl_commands_from_history(history_content.as_ref())
}

pub(crate) fn import_from_history() -> Result<Vec<String>> {
    let home = dirs::home_dir().unwrap_or_else(|| PathBuf::from("."));
    let history_files = [home.join(".bash_history"), home.join(".zsh_history")];

    let mut all_commands = Vec::new();
    let mut seen = HashSet::new();

    for history_file in history_files {
        if history_file.exists() {
            if let Ok(content) = fs::read(&history_file) {
                let commands = parse_curl_commands_from_history_bytes(&content);
                for cmd in commands {
                    if seen.insert(cmd.clone()) {
                        all_commands.push(cmd);
                    }
                }
            }
        }
    }

    Ok(all_commands)
}

#[cfg(test)]
mod tests {
    use super::{parse_curl_commands_from_history, parse_curl_commands_from_history_bytes};

    #[test]
    fn test_parse_curl_commands_from_bash_history() {
        let history_content = r#"ls -la
curl https://example.com
cd /home/user
curl -X POST https://api.github.com/repos
git status
  curl   https://httpbin.org/get  
echo "hello world"
curl -H "Authorization: Bearer token" https://api.example.com/data"#;

        let commands = parse_curl_commands_from_history(history_content);

        assert_eq!(commands.len(), 4);
        assert!(commands.contains(&"curl https://example.com".to_string()));
        assert!(commands.contains(&"curl -X POST https://api.github.com/repos".to_string()));
        assert!(commands.contains(&"curl   https://httpbin.org/get".to_string()));
        assert!(commands.contains(
            &"curl -H \"Authorization: Bearer token\" https://api.example.com/data".to_string()
        ));
    }

    #[test]
    fn test_parse_curl_commands_from_zsh_history() {
        let history_content = r#": 1647875000:0;ls -la
: 1647875010:0;curl https://example.com
: 1647875020:0;cd /home/user
: 1647875030:0;curl -X POST https://api.github.com/repos
: 1647875040:0;git status
: 1647875050:0;curl   -H "Content-Type: application/json" https://httpbin.org/post"#;

        let commands = parse_curl_commands_from_history(history_content);

        assert_eq!(commands.len(), 3);
        assert!(commands.contains(&"curl https://example.com".to_string()));
        assert!(commands.contains(&"curl -X POST https://api.github.com/repos".to_string()));
        assert!(commands.contains(
            &"curl   -H \"Content-Type: application/json\" https://httpbin.org/post".to_string()
        ));
    }

    #[test]
    fn test_parse_curl_commands_removes_duplicates() {
        let history_content = r#"curl https://example.com
curl https://github.com
curl https://example.com
curl https://example.com"#;

        let commands = parse_curl_commands_from_history(history_content);

        assert_eq!(commands.len(), 2);
        assert!(commands.contains(&"curl https://example.com".to_string()));
        assert!(commands.contains(&"curl https://github.com".to_string()));
    }

    #[test]
    fn test_parse_curl_commands_mixed_history_formats() {
        let history_content = r#"curl https://example1.com
: 1647875000:0;curl https://example2.com
curl -X POST https://example3.com
: 1647875010:0;curl -H "Auth: token" https://example4.com"#;

        let commands = parse_curl_commands_from_history(history_content);

        assert_eq!(commands.len(), 4);
        assert!(commands.contains(&"curl https://example1.com".to_string()));
        assert!(commands.contains(&"curl https://example2.com".to_string()));
        assert!(commands.contains(&"curl -X POST https://example3.com".to_string()));
        assert!(commands.contains(&"curl -H \"Auth: token\" https://example4.com".to_string()));
    }

    #[test]
    fn test_parse_curl_commands_from_non_utf8_history() {
        let history_bytes = b": 1647875000:0;curl https://example.com\n\x83\xffgarbage\n: 1647875001:0;curl -X POST https://api.github.com/repos\n";

        let commands = parse_curl_commands_from_history_bytes(history_bytes);

        assert_eq!(commands.len(), 2);
        assert!(commands.contains(&"curl https://example.com".to_string()));
        assert!(commands.contains(&"curl -X POST https://api.github.com/repos".to_string()));
    }
}
