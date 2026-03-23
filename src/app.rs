use crate::cli::build_cli;
use crate::config::{
    load_all_team_commands, load_team_commands, resolve_active_biblioteca, resolve_config,
    resolve_data_file_path, resolve_shared_storage_context, shared_repository_required_message,
    CombibConfig, DefaultSharedReadTarget, SharedStorageContext,
};
use crate::database::{CommandDatabase, StoredCommand};
use crate::Result;
use std::collections::HashSet;

const DEFAULT_LIST_LIMIT: usize = 20;
const DEFAULT_SHARED_SELECTION_REQUIRED_MESSAGE: &str =
    "No default shared selection configured. Use --team, --all-teams, or configure shared_repo.default_team / shared_repo.default_all_teams.";

#[derive(Debug, Clone, PartialEq, Eq)]
enum OutputSectionSource {
    Local { biblioteca: String },
    SharedTeam { team: String, biblioteca: String },
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct OutputSection {
    source: OutputSectionSource,
    entries: Vec<OutputEntry>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct OutputEntry {
    command: String,
    description: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum SharedReadTarget {
    Team(String),
    AllTeams,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct DefaultReadPlan {
    include_local: bool,
    shared_target: Option<SharedReadTarget>,
}

#[derive(Debug, Default, Clone, PartialEq, Eq)]
struct OutputSummary {
    hidden_local_duplicates: usize,
    hidden_due_to_limit: usize,
    active_limit: Option<usize>,
}

impl OutputSection {
    fn local(biblioteca: impl Into<String>, entries: Vec<OutputEntry>) -> Self {
        Self {
            source: OutputSectionSource::Local {
                biblioteca: biblioteca.into(),
            },
            entries,
        }
    }

    fn shared_team(
        team: impl Into<String>,
        biblioteca: impl Into<String>,
        entries: Vec<OutputEntry>,
    ) -> Self {
        Self {
            source: OutputSectionSource::SharedTeam {
                team: team.into(),
                biblioteca: biblioteca.into(),
            },
            entries,
        }
    }

    fn title(&self) -> String {
        match &self.source {
            OutputSectionSource::Local { biblioteca } => format!("Local / {biblioteca}"),
            OutputSectionSource::SharedTeam { team, biblioteca } => {
                format!("Shared / {team} / {biblioteca}")
            }
        }
    }

    fn is_shared(&self) -> bool {
        matches!(self.source, OutputSectionSource::SharedTeam { .. })
    }
}

impl OutputEntry {
    fn from_command(command: &StoredCommand) -> Self {
        Self {
            command: command.command.clone(),
            description: command.description.clone(),
        }
    }

    fn from_owned_command(command: StoredCommand) -> Self {
        Self {
            command: command.command,
            description: command.description,
        }
    }
}

impl From<DefaultSharedReadTarget> for SharedReadTarget {
    fn from(value: DefaultSharedReadTarget) -> Self {
        match value {
            DefaultSharedReadTarget::Team(team) => Self::Team(team),
            DefaultSharedReadTarget::AllTeams => Self::AllTeams,
        }
    }
}

pub fn run() -> Result<()> {
    if std::env::args_os().len() == 1 {
        let mut cmd = build_cli();
        cmd.print_help()?;
        println!();
        return Ok(());
    }

    let matches = build_cli().get_matches();
    let config = resolve_config(&matches)?;
    validate_matches(&matches)?;

    let all_teams = matches.get_flag("all-teams");
    let shared_context = resolve_shared_storage_context(&matches, &config)?;
    let biblioteca = resolve_active_biblioteca(&matches, &config)?;
    let data_file = resolve_data_file_path(&matches, shared_context.as_ref(), &biblioteca)?;

    if let Some(command) = matches.get_one::<String>("add") {
        let description = matches
            .get_one::<String>("description")
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty());
        let mut db = CommandDatabase::load_from_file(&data_file)?;
        let added = db.add_command(command.clone(), description.clone());
        if added {
            db.save_to_file(&data_file)?;
            match description {
                Some(description) => {
                    println!(
                        "Added command to biblioteca '{biblioteca}': {command} ({description})"
                    );
                }
                None => println!("Added command to biblioteca '{biblioteca}': {command}"),
            }
        } else {
            println!("Command already exists in biblioteca '{biblioteca}'.");
        }
        return Ok(());
    }

    if matches.get_flag("list") {
        let list_keywords: Option<Vec<String>> = matches
            .get_many::<String>("keywords")
            .map(|keywords| keywords.cloned().collect());
        let limit = resolve_list_limit(&matches, &config);

        if let Some(team) = matches.get_one::<String>("team") {
            let commands = CommandDatabase::load_from_file(&data_file)?;
            let mut sections = vec![OutputSection::shared_team(
                team.clone(),
                biblioteca.clone(),
                filter_commands(&commands, list_keywords.as_deref()),
            )];
            let summary = OutputSummary {
                hidden_due_to_limit: apply_list_limit(&mut sections, limit),
                active_limit: limit,
                ..OutputSummary::default()
            };
            print_sections(
                &sections,
                &empty_message(list_keywords.is_some(), &biblioteca),
                &summary,
            );
            return Ok(());
        }

        if all_teams {
            let mut sections = load_shared_sections_for_target(
                shared_context
                    .as_ref()
                    .ok_or(shared_repository_required_message())?,
                &SharedReadTarget::AllTeams,
                &biblioteca,
                list_keywords.as_deref(),
            )?;
            let summary = OutputSummary {
                hidden_due_to_limit: apply_list_limit(&mut sections, limit),
                active_limit: limit,
                ..OutputSummary::default()
            };
            print_sections(
                &sections,
                &empty_message(list_keywords.is_some(), &biblioteca),
                &summary,
            );
            return Ok(());
        }

        let local_db = CommandDatabase::load_from_file(&data_file)?;
        let plan = resolve_default_read_plan(&matches, &config, shared_context.as_ref())?;
        let (mut sections, hidden_local_duplicates) = load_default_read_sections(
            &local_db,
            shared_context.as_ref(),
            &biblioteca,
            list_keywords.as_deref(),
            &plan,
        )?;
        let mut summary = OutputSummary {
            hidden_local_duplicates,
            hidden_due_to_limit: 0,
            active_limit: limit,
        };
        summary.hidden_due_to_limit = apply_list_limit(&mut sections, limit);
        print_sections(
            &sections,
            &empty_message(list_keywords.is_some(), &biblioteca),
            &summary,
        );
        return Ok(());
    }

    if let Some(keywords) = matches.get_many::<String>("keywords") {
        let keyword_vec: Vec<String> = keywords.cloned().collect();

        if let Some(team) = matches.get_one::<String>("team") {
            let commands = CommandDatabase::load_from_file(&data_file)?;
            let sections = vec![OutputSection::shared_team(
                team.clone(),
                biblioteca.clone(),
                filter_commands(&commands, Some(&keyword_vec)),
            )];
            print_sections(
                &sections,
                &empty_message(true, &biblioteca),
                &OutputSummary::default(),
            );
            return Ok(());
        }

        if all_teams {
            let sections = load_shared_sections_for_target(
                shared_context
                    .as_ref()
                    .ok_or(shared_repository_required_message())?,
                &SharedReadTarget::AllTeams,
                &biblioteca,
                Some(&keyword_vec),
            )?;
            print_sections(
                &sections,
                &empty_message(true, &biblioteca),
                &OutputSummary::default(),
            );
            return Ok(());
        }

        let local_db = CommandDatabase::load_from_file(&data_file)?;
        let plan = resolve_default_read_plan(&matches, &config, shared_context.as_ref())?;
        let (sections, hidden_local_duplicates) = load_default_read_sections(
            &local_db,
            shared_context.as_ref(),
            &biblioteca,
            Some(&keyword_vec),
            &plan,
        )?;
        let summary = OutputSummary {
            hidden_local_duplicates,
            hidden_due_to_limit: 0,
            active_limit: None,
        };
        print_sections(&sections, &empty_message(true, &biblioteca), &summary);
    }

    Ok(())
}

fn validate_matches(matches: &clap::ArgMatches) -> Result<()> {
    let all_teams = matches.get_flag("all-teams");
    let local_only = matches.get_flag("local-only");
    let shared_only = matches.get_flag("shared-only");

    if local_only && shared_only {
        return Err("--local-only cannot be used together with --shared-only.".into());
    }

    if matches.get_one::<String>("team").is_some() && (local_only || shared_only) {
        return Err("--local-only and --shared-only cannot be used with --team.".into());
    }

    if all_teams && (local_only || shared_only) {
        return Err("--local-only and --shared-only cannot be used with --all-teams.".into());
    }

    if matches.get_one::<usize>("limit").is_some() && !matches.get_flag("list") {
        return Err("--limit can only be used with --list.".into());
    }

    if matches.get_one::<String>("description").is_some()
        && matches.get_one::<String>("add").is_none()
    {
        return Err("--description can only be used with --add.".into());
    }

    if matches.get_one::<String>("add").is_some() {
        if all_teams {
            return Err("--all-teams cannot be used with --add.".into());
        }
        if local_only || shared_only {
            return Err("--local-only and --shared-only cannot be used with --add.".into());
        }
        if matches.get_one::<String>("repo").is_some()
            && matches.get_one::<String>("team").is_none()
        {
            return Err("--repo requires --team when using shared repository write mode.".into());
        }
        if matches.get_one::<String>("teams-dir").is_some()
            && matches.get_one::<String>("team").is_none()
        {
            return Err(
                "--teams-dir requires --team when using shared repository write mode.".into(),
            );
        }
    }
    Ok(())
}

fn filter_commands(database: &CommandDatabase, keywords: Option<&[String]>) -> Vec<OutputEntry> {
    match keywords {
        Some(keywords) => database
            .search(keywords)
            .into_iter()
            .map(OutputEntry::from_command)
            .collect(),
        None => database
            .commands
            .iter()
            .map(OutputEntry::from_command)
            .collect(),
    }
}

fn resolve_default_read_plan(
    matches: &clap::ArgMatches,
    config: &CombibConfig,
    shared_context: Option<&SharedStorageContext>,
) -> Result<DefaultReadPlan> {
    if matches.get_flag("local-only") {
        return Ok(DefaultReadPlan {
            include_local: true,
            shared_target: None,
        });
    }

    if matches.get_flag("shared-only") {
        if shared_context.is_none() {
            return Err(shared_repository_required_message().into());
        }
        return Ok(DefaultReadPlan {
            include_local: false,
            shared_target: Some(
                config
                    .default_shared_read_target()
                    .ok_or(DEFAULT_SHARED_SELECTION_REQUIRED_MESSAGE)?
                    .into(),
            ),
        });
    }

    Ok(DefaultReadPlan {
        include_local: true,
        shared_target: if shared_context.is_some() {
            config.default_shared_read_target().map(Into::into)
        } else {
            None
        },
    })
}

fn load_default_read_sections(
    local_db: &CommandDatabase,
    shared_context: Option<&SharedStorageContext>,
    biblioteca: &str,
    keywords: Option<&[String]>,
    plan: &DefaultReadPlan,
) -> Result<(Vec<OutputSection>, usize)> {
    let mut local_commands = if plan.include_local {
        filter_commands(local_db, keywords)
    } else {
        Vec::new()
    };

    let shared_sections = match &plan.shared_target {
        Some(target) => load_shared_sections_for_target(
            shared_context.ok_or(shared_repository_required_message())?,
            target,
            biblioteca,
            keywords,
        )?,
        None => Vec::new(),
    };

    let hidden_local_duplicates =
        hide_local_duplicates(&mut local_commands, shared_sections.as_slice());

    let mut sections = Vec::new();
    if !local_commands.is_empty() {
        sections.push(OutputSection::local(biblioteca.to_string(), local_commands));
    }
    sections.extend(shared_sections);

    Ok((sections, hidden_local_duplicates))
}

fn load_shared_sections_for_target(
    shared_context: &SharedStorageContext,
    target: &SharedReadTarget,
    biblioteca: &str,
    keywords: Option<&[String]>,
) -> Result<Vec<OutputSection>> {
    match target {
        SharedReadTarget::Team(team) => {
            let commands = load_team_commands(shared_context, team, biblioteca, keywords)?;
            Ok(vec![OutputSection::shared_team(
                team.clone(),
                biblioteca.to_string(),
                commands
                    .into_iter()
                    .map(OutputEntry::from_owned_command)
                    .collect(),
            )])
        }
        SharedReadTarget::AllTeams => load_shared_sections(shared_context, biblioteca, keywords),
    }
}

fn load_shared_sections(
    shared_context: &SharedStorageContext,
    biblioteca: &str,
    keywords: Option<&[String]>,
) -> Result<Vec<OutputSection>> {
    let results = load_all_team_commands(shared_context, biblioteca, keywords)?;
    let mut sections = Vec::new();
    let mut current_team = None::<String>;
    let mut current_commands = Vec::new();

    for (team, command) in results {
        if current_team.as_deref() != Some(team.as_str()) {
            if let Some(team_name) = current_team.take() {
                sections.push(OutputSection::shared_team(
                    team_name,
                    biblioteca.to_string(),
                    std::mem::take(&mut current_commands),
                ));
            }
            current_team = Some(team);
        }
        current_commands.push(OutputEntry::from_owned_command(command));
    }

    if let Some(team_name) = current_team {
        sections.push(OutputSection::shared_team(
            team_name,
            biblioteca.to_string(),
            current_commands,
        ));
    }

    Ok(sections)
}

fn hide_local_duplicates(
    local_commands: &mut Vec<OutputEntry>,
    shared_sections: &[OutputSection],
) -> usize {
    let shared_commands: HashSet<&str> = shared_sections
        .iter()
        .filter(|section| section.is_shared())
        .flat_map(|section| section.entries.iter().map(|entry| entry.command.as_str()))
        .collect();

    if shared_commands.is_empty() {
        return 0;
    }

    let original_len = local_commands.len();
    local_commands.retain(|command| !shared_commands.contains(command.command.as_str()));
    original_len.saturating_sub(local_commands.len())
}

fn resolve_list_limit(matches: &clap::ArgMatches, config: &CombibConfig) -> Option<usize> {
    if let Some(limit) = matches.get_one::<usize>("limit").copied() {
        return normalize_limit(limit);
    }

    match config.default_list_limit {
        Some(limit) => normalize_limit(limit),
        None => Some(DEFAULT_LIST_LIMIT),
    }
}

fn normalize_limit(limit: usize) -> Option<usize> {
    if limit == 0 {
        None
    } else {
        Some(limit)
    }
}

fn apply_list_limit(sections: &mut [OutputSection], limit: Option<usize>) -> usize {
    let Some(mut remaining) = limit else {
        return 0;
    };

    let mut hidden = 0;
    for section in sections {
        if remaining == 0 {
            hidden += section.entries.len();
            section.entries.clear();
            continue;
        }

        if section.entries.len() > remaining {
            hidden += section.entries.len() - remaining;
            section.entries.truncate(remaining);
            remaining = 0;
        } else {
            remaining -= section.entries.len();
        }
    }

    hidden
}

fn empty_message(filtered: bool, biblioteca: &str) -> String {
    if filtered {
        format!("No matching commands in biblioteca '{biblioteca}'.")
    } else {
        format!("No commands stored in biblioteca '{biblioteca}'.")
    }
}

fn print_sections(sections: &[OutputSection], empty_message: &str, summary: &OutputSummary) {
    let sections: Vec<&OutputSection> = sections
        .iter()
        .filter(|section| !section.entries.is_empty())
        .collect();

    if sections.is_empty() {
        println!("{empty_message}");
        return;
    }

    for (section_index, section) in sections.iter().enumerate() {
        if section_index > 0 {
            println!();
        }

        println!("{}", format_section_header(&section.title()));
        println!();

        for (index, entry) in section.entries.iter().enumerate() {
            if index > 0 {
                println!();
            }

            match entry.description.as_deref() {
                Some(description) => println!("[{}] {}", index + 1, description),
                None => println!("[{}]", index + 1),
            }
            println!("{}", entry.command);
        }
    }

    let duplicate_message = format_duplicate_hidden_message(summary.hidden_local_duplicates);
    let limit_message =
        format_limit_hidden_message(summary.hidden_due_to_limit, summary.active_limit);

    if duplicate_message.is_some() || limit_message.is_some() {
        println!();
    }

    if let Some(message) = duplicate_message {
        println!("{message}");
    }

    if let Some(message) = limit_message {
        println!("{message}");
    }
}

fn format_section_header(title: &str) -> String {
    format!("=== {} ===", title.to_uppercase())
}

fn format_duplicate_hidden_message(hidden_local_duplicates: usize) -> Option<String> {
    if hidden_local_duplicates == 0 {
        None
    } else if hidden_local_duplicates == 1 {
        Some("1 local command was hidden because it duplicates shared storage.".to_string())
    } else {
        Some(format!(
            "{hidden_local_duplicates} local commands were hidden because they duplicate shared storage."
        ))
    }
}

fn format_limit_hidden_message(
    hidden_due_to_limit: usize,
    active_limit: Option<usize>,
) -> Option<String> {
    let limit = active_limit?;

    if hidden_due_to_limit == 0 {
        None
    } else if hidden_due_to_limit == 1 {
        Some(format!(
            "Showing first {limit} commands. 1 additional command was hidden by the active list limit."
        ))
    } else {
        Some(format!(
            "Showing first {limit} commands. {hidden_due_to_limit} additional commands were hidden by the active list limit."
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::{
        apply_list_limit, format_duplicate_hidden_message, format_limit_hidden_message,
        format_section_header, hide_local_duplicates, normalize_limit, OutputEntry, OutputSection,
    };

    #[test]
    fn test_format_section_header_for_local() {
        assert_eq!(
            format_section_header("Local / curl"),
            "=== LOCAL / CURL ==="
        );
    }

    #[test]
    fn test_format_section_header_for_shared_team() {
        assert_eq!(
            format_section_header("Shared / platform / curl"),
            "=== SHARED / PLATFORM / CURL ==="
        );
    }

    #[test]
    fn test_hide_local_duplicates_against_shared_sections() {
        let mut local_commands = vec![
            OutputEntry {
                command: "curl https://shared.example.com/health".to_string(),
                description: Some("Shared health".to_string()),
            },
            OutputEntry {
                command: "curl https://local.example.com/health".to_string(),
                description: Some("Local health".to_string()),
            },
        ];
        let shared_sections = vec![OutputSection::shared_team(
            "platform",
            "curl",
            vec![OutputEntry {
                command: "curl https://shared.example.com/health".to_string(),
                description: Some("Shared health".to_string()),
            }],
        )];

        let hidden = hide_local_duplicates(&mut local_commands, &shared_sections);

        assert_eq!(hidden, 1);
        assert_eq!(
            local_commands,
            vec![OutputEntry {
                command: "curl https://local.example.com/health".to_string(),
                description: Some("Local health".to_string()),
            }]
        );
    }

    #[test]
    fn test_apply_list_limit_across_sections() {
        let mut sections = vec![
            OutputSection::local(
                "curl",
                vec![
                    OutputEntry {
                        command: "curl https://local.example.com/one".to_string(),
                        description: None,
                    },
                    OutputEntry {
                        command: "curl https://local.example.com/two".to_string(),
                        description: Some("Second".to_string()),
                    },
                ],
            ),
            OutputSection::shared_team(
                "platform",
                "curl",
                vec![OutputEntry {
                    command: "curl https://shared.example.com/one".to_string(),
                    description: None,
                }],
            ),
        ];

        let hidden = apply_list_limit(&mut sections, Some(2));

        assert_eq!(hidden, 1);
        assert_eq!(sections[0].entries.len(), 2);
        assert!(sections[1].entries.is_empty());
    }

    #[test]
    fn test_normalize_limit_zero_means_unlimited() {
        assert_eq!(normalize_limit(0), None);
        assert_eq!(normalize_limit(5), Some(5));
    }

    #[test]
    fn test_duplicate_hidden_message_pluralization() {
        assert_eq!(
            format_duplicate_hidden_message(1),
            Some("1 local command was hidden because it duplicates shared storage.".to_string())
        );
        assert_eq!(
            format_duplicate_hidden_message(2),
            Some("2 local commands were hidden because they duplicate shared storage.".to_string())
        );
    }

    #[test]
    fn test_limit_hidden_message_pluralization() {
        assert_eq!(
            format_limit_hidden_message(1, Some(20)),
            Some(
                "Showing first 20 commands. 1 additional command was hidden by the active list limit."
                    .to_string()
            )
        );
        assert_eq!(
            format_limit_hidden_message(3, Some(10)),
            Some(
                "Showing first 10 commands. 3 additional commands were hidden by the active list limit."
                    .to_string()
            )
        );
    }
}
