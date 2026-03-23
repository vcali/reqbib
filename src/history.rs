use crate::Result;
use regex::Regex;
use std::collections::HashSet;
use std::fs;
use std::path::PathBuf;
use std::sync::OnceLock;

fn history_curl_regex() -> &'static Regex {
    static HISTORY_CURL_REGEX: OnceLock<Regex> = OnceLock::new();
    HISTORY_CURL_REGEX
        .get_or_init(|| Regex::new(r"(?s)^(\s*curl(?:\s+.*)?)$").expect("valid history regex"))
}

fn zsh_extended_history_prefix_regex() -> &'static Regex {
    static ZSH_EXTENDED_HISTORY_PREFIX_REGEX: OnceLock<Regex> = OnceLock::new();
    ZSH_EXTENDED_HISTORY_PREFIX_REGEX
        .get_or_init(|| Regex::new(r"^: \d+:\d+;(.*)$").expect("valid zsh history regex"))
}

fn strip_zsh_extended_history_prefix(line: &str) -> &str {
    zsh_extended_history_prefix_regex()
        .captures(line)
        .and_then(|captures| captures.get(1).map(|matched| matched.as_str()))
        .unwrap_or(line)
}

fn ends_with_continuation_marker(line: &str) -> bool {
    line.trim_end().ends_with('\\')
}

fn normalize_history_line(line: &str) -> String {
    let trimmed = line.trim_end();
    trimmed
        .strip_suffix('\\')
        .unwrap_or(trimmed)
        .trim_end()
        .to_string()
}

fn reconstruct_history_commands(history_content: &str) -> Vec<String> {
    let mut commands = Vec::new();
    let mut lines = history_content.lines();

    while let Some(line) = lines.next() {
        let mut clean_line = strip_zsh_extended_history_prefix(line);
        let mut command = normalize_history_line(clean_line);

        while ends_with_continuation_marker(clean_line) {
            let Some(next_line) = lines.next() else {
                break;
            };
            clean_line = strip_zsh_extended_history_prefix(next_line);
            command.push('\n');
            command.push_str(&normalize_history_line(clean_line));
        }

        commands.push(command);
    }

    commands
}

pub(crate) fn parse_curl_commands_from_history(history_content: &str) -> Vec<String> {
    let mut curl_commands = Vec::new();
    let mut seen = HashSet::new();

    for reconstructed_command in reconstruct_history_commands(history_content) {
        if let Some(cap) = history_curl_regex().captures(&reconstructed_command) {
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

    #[test]
    fn test_parse_curl_commands_from_multiline_bash_history() {
        let history_content = r#"curl -X POST https://api.example.com/graphql \
  -H "Content-Type: application/json" \
  -d '{"query":"{ viewer { login } }"}'
echo "done""#;

        let commands = parse_curl_commands_from_history(history_content);

        assert_eq!(commands.len(), 1);
        assert_eq!(
            commands[0],
            "curl -X POST https://api.example.com/graphql\n  -H \"Content-Type: application/json\"\n  -d '{\"query\":\"{ viewer { login } }\"}'"
        );
        assert!(!commands[0].contains('\\'));
    }

    #[test]
    fn test_parse_curl_commands_from_multiline_zsh_history() {
        let history_content = r#": 1647875010:0;curl -X POST https://api.example.com/graphql \
  -H "Content-Type: application/json" \
  -d '{"query":"{ viewer { login } }"}'
: 1647875020:0;echo "done""#;

        let commands = parse_curl_commands_from_history(history_content);

        assert_eq!(commands.len(), 1);
        assert_eq!(
            commands[0],
            "curl -X POST https://api.example.com/graphql\n  -H \"Content-Type: application/json\"\n  -d '{\"query\":\"{ viewer { login } }\"}'"
        );
        assert!(!commands[0].contains('\\'));
    }
}
