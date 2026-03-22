use crate::cli::build_cli;
use crate::config::{
    load_all_team_commands, resolve_config, resolve_data_file_path, resolve_shared_storage_context,
    shared_repository_required_message, ReadScopePreference, ReqbibConfig, SharedStorageContext,
};
use crate::database::CurlDatabase;
use crate::history::import_from_history;
use crate::Result;

#[derive(Debug, Clone, PartialEq, Eq)]
struct OutputSection {
    title: String,
    commands: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ReadScope {
    LocalOnly,
    SharedOnly,
    Combined,
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
    let all_teams = matches.get_flag("all-teams");
    let local_only = matches.get_flag("local-only");
    let shared_only = matches.get_flag("shared-only");
    let shared_context = resolve_shared_storage_context(&matches, &config)?;

    if local_only && shared_only {
        return Err("--local-only cannot be used together with --shared-only.".into());
    }

    if matches.get_one::<String>("team").is_some() && (local_only || shared_only) {
        return Err("--local-only and --shared-only cannot be used with --team.".into());
    }

    if all_teams && (local_only || shared_only) {
        return Err("--local-only and --shared-only cannot be used with --all-teams.".into());
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

    if matches.get_flag("import") {
        if all_teams {
            return Err("--all-teams cannot be used with --import.".into());
        }
        if local_only || shared_only {
            return Err("--local-only and --shared-only cannot be used with --import.".into());
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

    let data_file = resolve_data_file_path(&matches, shared_context.as_ref())?;
    let mut db = CurlDatabase::load_from_file(&data_file)?;

    if let Some(curl_command) = matches.get_one::<String>("add") {
        db.add_command(curl_command.clone());
        db.save_to_file(&data_file)?;
        println!("Added curl command: {}", curl_command);
    } else if matches.get_flag("import") {
        match import_from_history() {
            Ok(commands) => {
                let added_count = db.add_commands(commands);
                db.save_to_file(&data_file)?;
                println!(
                    "Imported {} new curl commands from shell history",
                    added_count
                );
            }
            Err(error) => {
                eprintln!("Error importing from history: {}", error);
            }
        }
    } else if matches.get_flag("list") {
        let list_keywords: Option<Vec<String>> = matches
            .get_many::<String>("keywords")
            .map(|keywords| keywords.cloned().collect());

        if let Some(team) = matches.get_one::<String>("team") {
            let sections = vec![OutputSection {
                title: format!("Shared / {}", team),
                commands: match list_keywords.as_deref() {
                    Some(keywords) => db
                        .search(keywords)
                        .into_iter()
                        .map(|cmd| cmd.command.clone())
                        .collect(),
                    None => db.commands.into_iter().map(|cmd| cmd.command).collect(),
                },
            }];
            print_sections(
                &sections,
                if list_keywords.is_some() {
                    "No matching curl commands."
                } else {
                    "No curl commands stored."
                },
            );
            return Ok(());
        }

        if all_teams {
            let sections = load_shared_sections(
                shared_context
                    .as_ref()
                    .ok_or(shared_repository_required_message())?,
                list_keywords.as_deref(),
            )?;
            print_sections(
                &sections,
                if list_keywords.is_some() {
                    "No matching curl commands."
                } else {
                    "No curl commands stored."
                },
            );
            return Ok(());
        }

        let scope = resolve_read_scope(&matches, &config, shared_context.as_ref())?;
        let sections = load_default_read_sections(
            &db,
            shared_context.as_ref(),
            list_keywords.as_deref(),
            scope,
        )?;
        print_sections(
            &sections,
            if list_keywords.is_some() {
                "No matching curl commands."
            } else {
                "No curl commands stored."
            },
        );
    } else if let Some(keywords) = matches.get_many::<String>("keywords") {
        let keyword_vec: Vec<String> = keywords.cloned().collect();

        if let Some(team) = matches.get_one::<String>("team") {
            let sections = vec![OutputSection {
                title: format!("Shared / {}", team),
                commands: db
                    .search(&keyword_vec)
                    .into_iter()
                    .map(|cmd| cmd.command.clone())
                    .collect(),
            }];
            print_sections(&sections, "No matching curl commands.");
            return Ok(());
        }

        if all_teams {
            let sections = load_shared_sections(
                shared_context
                    .as_ref()
                    .ok_or(shared_repository_required_message())?,
                Some(&keyword_vec),
            )?;
            print_sections(&sections, "No matching curl commands.");
            return Ok(());
        }

        let scope = resolve_read_scope(&matches, &config, shared_context.as_ref())?;
        let sections =
            load_default_read_sections(&db, shared_context.as_ref(), Some(&keyword_vec), scope)?;
        print_sections(&sections, "No matching curl commands.");
    }

    Ok(())
}

fn resolve_read_scope(
    matches: &clap::ArgMatches,
    config: &ReqbibConfig,
    shared_context: Option<&SharedStorageContext>,
) -> Result<ReadScope> {
    if matches.get_flag("local-only") {
        return Ok(ReadScope::LocalOnly);
    }

    if matches.get_flag("shared-only") {
        if shared_context.is_none() {
            return Err(shared_repository_required_message().into());
        }
        return Ok(ReadScope::SharedOnly);
    }

    match config.default_read_scope {
        Some(ReadScopePreference::Local) => Ok(ReadScope::LocalOnly),
        Some(ReadScopePreference::Shared) => {
            if shared_context.is_none() {
                Err(shared_repository_required_message().into())
            } else {
                Ok(ReadScope::SharedOnly)
            }
        }
        Some(ReadScopePreference::Combined) => {
            if shared_context.is_some() {
                Ok(ReadScope::Combined)
            } else {
                Ok(ReadScope::LocalOnly)
            }
        }
        None => {
            if shared_context.is_some() {
                Ok(ReadScope::Combined)
            } else {
                Ok(ReadScope::LocalOnly)
            }
        }
    }
}

fn load_default_read_sections(
    local_db: &CurlDatabase,
    shared_context: Option<&SharedStorageContext>,
    keywords: Option<&[String]>,
    scope: ReadScope,
) -> Result<Vec<OutputSection>> {
    let mut sections = Vec::new();

    if matches!(scope, ReadScope::LocalOnly | ReadScope::Combined) {
        let local_commands: Vec<String> = match keywords {
            Some(keywords) => local_db
                .search(keywords)
                .into_iter()
                .map(|cmd| cmd.command.clone())
                .collect(),
            None => local_db
                .commands
                .iter()
                .map(|cmd| cmd.command.clone())
                .collect(),
        };

        if !local_commands.is_empty() {
            sections.push(OutputSection {
                title: "Local".to_string(),
                commands: local_commands,
            });
        }
    }

    if matches!(scope, ReadScope::SharedOnly | ReadScope::Combined) {
        let shared_context = shared_context.ok_or(shared_repository_required_message())?;
        sections.extend(load_shared_sections(shared_context, keywords)?);
    }

    Ok(sections)
}

fn load_shared_sections(
    shared_context: &SharedStorageContext,
    keywords: Option<&[String]>,
) -> Result<Vec<OutputSection>> {
    let results = load_all_team_commands(shared_context, keywords)?;
    let mut sections = Vec::new();
    let mut current_team = None::<String>;
    let mut current_commands = Vec::new();

    for (team, command) in results {
        if current_team.as_deref() != Some(team.as_str()) {
            if let Some(team_name) = current_team.take() {
                sections.push(OutputSection {
                    title: format!("Shared / {}", team_name),
                    commands: std::mem::take(&mut current_commands),
                });
            }
            current_team = Some(team);
        }
        current_commands.push(command);
    }

    if let Some(team_name) = current_team {
        sections.push(OutputSection {
            title: format!("Shared / {}", team_name),
            commands: current_commands,
        });
    }

    Ok(sections)
}

fn print_sections(sections: &[OutputSection], empty_message: &str) {
    let sections: Vec<&OutputSection> = sections
        .iter()
        .filter(|section| !section.commands.is_empty())
        .collect();

    if sections.is_empty() {
        println!("{}", empty_message);
        return;
    }

    for (section_index, section) in sections.iter().enumerate() {
        if section_index > 0 {
            println!();
        }

        println!("{}", section.title);
        println!();

        for (index, command) in section.commands.iter().enumerate() {
            if index > 0 {
                println!();
            }

            println!("[{}]", index + 1);
            println!("{}", command);
        }
    }
}
