use crate::cli::build_cli;
use crate::config::{
    get_local_data_file_path, list_all_team_shelves, list_local_shelves, list_team_shelves,
    load_all_team_commands, load_team_commands, resolve_active_shelf, resolve_config,
    resolve_data_file_path, resolve_shared_storage_context, shared_repository_required_message,
    DefaultSharedReadTarget, SharedStorageContext, ShellshelfConfig,
};
use crate::database::{CommandDatabase, StoredCommand};
use crate::postman_import::{import_postman_collection, PostmanImportOutcome};
use crate::Result;
use std::collections::HashSet;
use std::path::Path;

const DEFAULT_LIST_LIMIT: usize = 20;
const DEFAULT_SHARED_SELECTION_REQUIRED_MESSAGE: &str =
    "No default shared selection configured. Use --team, --all-teams, or configure shared_repo.default_team / shared_repo.default_all_teams.";

#[derive(Debug, Clone, PartialEq, Eq)]
enum OutputSectionSource {
    Local { shelf: String },
    SharedTeam { team: String, shelf: String },
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

#[derive(Debug, Clone, PartialEq, Eq)]
struct ShelfSection {
    title: String,
    shelves: Vec<String>,
}

impl OutputSection {
    fn local(shelf: impl Into<String>, entries: Vec<OutputEntry>) -> Self {
        Self {
            source: OutputSectionSource::Local {
                shelf: shelf.into(),
            },
            entries,
        }
    }

    fn shared_team(
        team: impl Into<String>,
        shelf: impl Into<String>,
        entries: Vec<OutputEntry>,
    ) -> Self {
        Self {
            source: OutputSectionSource::SharedTeam {
                team: team.into(),
                shelf: shelf.into(),
            },
            entries,
        }
    }

    fn title(&self) -> String {
        match &self.source {
            OutputSectionSource::Local { shelf } => format!("Local / {shelf}"),
            OutputSectionSource::SharedTeam { team, shelf } => {
                format!("Shared / {team} / {shelf}")
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
    let add_command = matches.get_one::<String>("add");
    let import_postman_path = matches.get_one::<String>("import-postman");
    let list_commands = matches.get_flag("list");
    let search_keywords: Option<Vec<String>> = matches
        .get_many::<String>("keywords")
        .map(|keywords| keywords.cloned().collect());

    let all_teams = matches.get_flag("all-teams");
    let shared_context = resolve_shared_storage_context(&matches, &config)?;
    let list_shelves = matches.get_flag("list-shelves");
    let needs_resolved_shelf = !list_shelves
        && import_postman_path.is_none()
        && (matches.get_one::<String>("create-shelf").is_some()
            || add_command.is_some()
            || list_commands
            || matches.get_one::<String>("shelf").is_some());
    let shelf = if list_shelves {
        None
    } else if needs_resolved_shelf {
        Some(resolve_target_shelf(&matches, &config)?)
    } else {
        None
    };
    let data_file = if let Some(shelf) = shelf.as_deref() {
        Some(resolve_data_file_path(
            &matches,
            shared_context.as_ref(),
            shelf,
        )?)
    } else {
        None
    };

    if list_shelves {
        return list_shelves_for_scope(&matches, &config, shared_context.as_ref());
    }

    if matches.get_one::<String>("create-shelf").is_some() {
        return create_shelf(
            &matches,
            data_file
                .as_deref()
                .expect("data file should be resolved for shelf creation"),
            shelf
                .as_deref()
                .expect("shelf should be resolved for shelf creation"),
        );
    }

    if let Some(import_path) = import_postman_path {
        return import_postman_shelf(&matches, shared_context.as_ref(), Path::new(import_path));
    }

    if let Some(command) = add_command {
        let shelf = shelf
            .as_deref()
            .expect("shelf should be resolved for add operations");
        let data_file = data_file
            .as_deref()
            .expect("data file should be resolved for add operations");
        let description = matches
            .get_one::<String>("description")
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty());
        let mut db = CommandDatabase::load_from_file(data_file)?;
        let added = db.add_command(command.clone(), description.clone());
        if added {
            db.save_to_file(data_file)?;
            match description {
                Some(description) => {
                    println!("Added command to shelf '{shelf}': {command} ({description})");
                }
                None => println!("Added command to shelf '{shelf}': {command}"),
            }
        } else {
            println!("Command already exists in shelf '{shelf}'.");
        }
        return Ok(());
    }

    if list_commands {
        let shelf = shelf
            .as_deref()
            .expect("shelf should be resolved for list operations");
        let data_file = data_file
            .as_deref()
            .expect("data file should be resolved for list operations");
        let limit = resolve_list_limit(&matches, &config);

        if let Some(team) = matches.get_one::<String>("team") {
            let commands = CommandDatabase::load_from_file(data_file)?;
            let mut sections = vec![OutputSection::shared_team(
                team.clone(),
                shelf.to_string(),
                filter_commands(&commands, search_keywords.as_deref()),
            )];
            let summary = OutputSummary {
                hidden_due_to_limit: apply_list_limit(&mut sections, limit),
                active_limit: limit,
                ..OutputSummary::default()
            };
            print_sections(
                &sections,
                &empty_message(search_keywords.is_some(), Some(shelf)),
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
                shelf,
                search_keywords.as_deref(),
            )?;
            let summary = OutputSummary {
                hidden_due_to_limit: apply_list_limit(&mut sections, limit),
                active_limit: limit,
                ..OutputSummary::default()
            };
            print_sections(
                &sections,
                &empty_message(search_keywords.is_some(), Some(shelf)),
                &summary,
            );
            return Ok(());
        }

        let local_db = CommandDatabase::load_from_file(data_file)?;
        let plan = resolve_default_read_plan(&matches, &config, shared_context.as_ref())?;
        let (mut sections, hidden_local_duplicates) = load_default_read_sections(
            &local_db,
            shared_context.as_ref(),
            shelf,
            search_keywords.as_deref(),
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
            &empty_message(search_keywords.is_some(), Some(shelf)),
            &summary,
        );
        return Ok(());
    }

    if let Some(keyword_vec) = search_keywords.as_deref() {
        if let Some(shelf) = shelf.as_deref() {
            if let Some(team) = matches.get_one::<String>("team") {
                let data_file = data_file
                    .as_deref()
                    .expect("data file should be resolved for team shelf search");
                let commands = CommandDatabase::load_from_file(data_file)?;
                let sections = vec![OutputSection::shared_team(
                    team.clone(),
                    shelf.to_string(),
                    filter_commands(&commands, Some(keyword_vec)),
                )];
                print_sections(
                    &sections,
                    &empty_message(true, Some(shelf)),
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
                    shelf,
                    Some(keyword_vec),
                )?;
                print_sections(
                    &sections,
                    &empty_message(true, Some(shelf)),
                    &OutputSummary::default(),
                );
                return Ok(());
            }

            let data_file = data_file
                .as_deref()
                .expect("data file should be resolved for single-shelf search");
            let local_db = CommandDatabase::load_from_file(data_file)?;
            let plan = resolve_default_read_plan(&matches, &config, shared_context.as_ref())?;
            let (sections, hidden_local_duplicates) = load_default_read_sections(
                &local_db,
                shared_context.as_ref(),
                shelf,
                Some(keyword_vec),
                &plan,
            )?;
            let summary = OutputSummary {
                hidden_local_duplicates,
                hidden_due_to_limit: 0,
                active_limit: None,
            };
            print_sections(&sections, &empty_message(true, Some(shelf)), &summary);
            return Ok(());
        }

        let (sections, hidden_local_duplicates) = load_search_sections_without_active_shelf(
            &matches,
            &config,
            shared_context.as_ref(),
            keyword_vec,
        )?;
        let summary = OutputSummary {
            hidden_local_duplicates,
            hidden_due_to_limit: 0,
            active_limit: None,
        };
        print_sections(&sections, &empty_message(true, None), &summary);
    }

    Ok(())
}

fn validate_matches(matches: &clap::ArgMatches) -> Result<()> {
    let all_teams = matches.get_flag("all-teams");
    let local_only = matches.get_flag("local-only");
    let shared_only = matches.get_flag("shared-only");
    let import_postman = matches.get_one::<String>("import-postman");

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

    if matches.get_flag("list-shelves") {
        if matches.get_one::<String>("add").is_some()
            || matches.get_flag("list")
            || matches.get_one::<String>("create-shelf").is_some()
            || import_postman.is_some()
        {
            return Err(
                "--list-shelves cannot be combined with --add, --list, --create-shelf, or --import-postman.".into(),
            );
        }
        if matches.get_one::<String>("description").is_some() {
            return Err("--description cannot be used with --list-shelves.".into());
        }
        if matches.get_one::<usize>("limit").is_some() {
            return Err("--limit cannot be used with --list-shelves.".into());
        }
        if matches.get_one::<String>("shelf").is_some() {
            return Err("--shelf cannot be used with --list-shelves.".into());
        }
        if matches
            .get_many::<String>("keywords")
            .map(|values| values.len() > 0)
            .unwrap_or(false)
        {
            return Err("--list-shelves cannot be combined with search keywords.".into());
        }
    }

    if let Some(create_shelf) = matches.get_one::<String>("create-shelf") {
        if all_teams {
            return Err("--all-teams cannot be used with --create-shelf.".into());
        }
        if local_only || shared_only {
            return Err(
                "--local-only and --shared-only cannot be used with --create-shelf.".into(),
            );
        }
        if matches.get_one::<String>("add").is_some()
            || matches.get_flag("list")
            || import_postman.is_some()
        {
            return Err(
                "--create-shelf cannot be combined with --add, --list, or --import-postman.".into(),
            );
        }
        if matches
            .get_many::<String>("keywords")
            .map(|values| values.len() > 0)
            .unwrap_or(false)
        {
            return Err("--create-shelf cannot be combined with search keywords.".into());
        }
        if matches.get_one::<String>("description").is_some() {
            return Err("--description cannot be used with --create-shelf.".into());
        }
        if matches.get_one::<usize>("limit").is_some() {
            return Err("--limit cannot be used with --create-shelf.".into());
        }
        if let Some(active_shelf) = matches.get_one::<String>("shelf") {
            if active_shelf != create_shelf {
                return Err("--shelf must match --create-shelf when both are provided.".into());
            }
        }
        if matches.get_one::<String>("repo").is_some()
            && matches.get_one::<String>("team").is_none()
        {
            return Err("--repo requires --team when creating a shared shelf.".into());
        }
        if matches.get_one::<String>("teams-dir").is_some()
            && matches.get_one::<String>("team").is_none()
        {
            return Err("--teams-dir requires --team when creating a shared shelf.".into());
        }
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
        if import_postman.is_some() {
            return Err("--add cannot be combined with --import-postman.".into());
        }
    }

    if import_postman.is_some() {
        if all_teams {
            return Err("--all-teams cannot be used with --import-postman.".into());
        }
        if local_only || shared_only {
            return Err(
                "--local-only and --shared-only cannot be used with --import-postman.".into(),
            );
        }
        if matches.get_flag("list") {
            return Err("--list cannot be combined with --import-postman.".into());
        }
        if matches.get_one::<String>("description").is_some() {
            return Err("--description cannot be used with --import-postman.".into());
        }
        if matches.get_one::<String>("shelf").is_some() {
            return Err(
                "--shelf cannot be used with --import-postman. Use --target-shelf instead.".into(),
            );
        }
        if matches.get_one::<usize>("limit").is_some() {
            return Err("--limit cannot be used with --import-postman.".into());
        }
        if matches
            .get_many::<String>("keywords")
            .map(|values| values.len() > 0)
            .unwrap_or(false)
        {
            return Err("--import-postman cannot be combined with search keywords.".into());
        }
        if matches.get_one::<String>("repo").is_some()
            && matches.get_one::<String>("team").is_none()
        {
            return Err("--repo requires --team when importing into shared storage.".into());
        }
        if matches.get_one::<String>("teams-dir").is_some()
            && matches.get_one::<String>("team").is_none()
        {
            return Err("--teams-dir requires --team when importing into shared storage.".into());
        }
    }
    Ok(())
}

fn resolve_target_shelf(matches: &clap::ArgMatches, config: &ShellshelfConfig) -> Result<String> {
    if let Some(create_shelf) = matches.get_one::<String>("create-shelf") {
        crate::config::validate_shelf_name(create_shelf)?;
        Ok(create_shelf.clone())
    } else {
        resolve_active_shelf(matches, config)
    }
}

fn create_shelf(
    matches: &clap::ArgMatches,
    data_file: &std::path::Path,
    shelf: &str,
) -> Result<()> {
    if data_file.exists() {
        println!("Shelf '{shelf}' already exists.");
        return Ok(());
    }

    CommandDatabase::new().save_to_file(data_file)?;

    if let Some(team) = matches.get_one::<String>("team") {
        println!("Created shelf '{shelf}' for team '{team}'.");
    } else {
        println!("Created shelf '{shelf}'.");
    }

    Ok(())
}

fn import_postman_shelf(
    matches: &clap::ArgMatches,
    shared_context: Option<&SharedStorageContext>,
    import_path: &Path,
) -> Result<()> {
    let outcome = import_postman_collection(
        import_path,
        matches
            .get_one::<String>("target-shelf")
            .map(String::as_str),
    )?;
    let data_file = resolve_data_file_path(matches, shared_context, &outcome.shelf_name)?;

    if data_file.exists() {
        return Err(format!(
            "Shelf '{}' already exists. Use --target-shelf <NAME> to choose a different shelf name for this import.",
            outcome.shelf_name
        )
        .into());
    }

    outcome.database.save_to_file(&data_file)?;
    print_postman_import_summary(matches, &outcome);
    Ok(())
}

fn print_postman_import_summary(matches: &clap::ArgMatches, outcome: &PostmanImportOutcome) {
    let imported_count = outcome.database.commands.len();
    let skipped_count = outcome.warnings.len();
    let imported_label = if imported_count == 1 {
        "request"
    } else {
        "requests"
    };
    let skipped_label = if skipped_count == 1 {
        "request"
    } else {
        "requests"
    };

    if let Some(team) = matches.get_one::<String>("team") {
        println!(
            "Imported {imported_count} {imported_label} into shelf '{}' for team '{}'. Skipped {skipped_count} {skipped_label}.",
            outcome.shelf_name, team
        );
    } else {
        println!(
            "Imported {imported_count} {imported_label} into shelf '{}'. Skipped {skipped_count} {skipped_label}.",
            outcome.shelf_name
        );
    }

    if !outcome.warnings.is_empty() {
        eprintln!(
            "Warning: skipped {} {} during Postman import.",
            skipped_count, skipped_label
        );
        for warning in &outcome.warnings {
            eprintln!("- {}: {}", warning.request_name, warning.reason);
        }
    }
}

fn list_shelves_for_scope(
    matches: &clap::ArgMatches,
    config: &ShellshelfConfig,
    shared_context: Option<&SharedStorageContext>,
) -> Result<()> {
    if let Some(team) = matches.get_one::<String>("team") {
        let shared_context = shared_context.ok_or(shared_repository_required_message())?;
        let sections = vec![ShelfSection {
            title: format!("Shared / {team}"),
            shelves: list_team_shelves(shared_context, team)?,
        }];
        print_shelf_sections(
            &sections,
            &format!("No shelves available for team '{team}'."),
        );
        return Ok(());
    }

    if matches.get_flag("all-teams") {
        let shared_context = shared_context.ok_or(shared_repository_required_message())?;
        let sections = sections_from_grouped_team_shelves(list_all_team_shelves(shared_context)?);
        print_shelf_sections(&sections, "No shelves available in shared storage.");
        return Ok(());
    }

    let plan = resolve_default_read_plan(matches, config, shared_context)?;
    let mut sections = Vec::new();

    if plan.include_local {
        sections.push(ShelfSection {
            title: "Local".to_string(),
            shelves: list_local_shelves()?,
        });
    }

    match plan.shared_target {
        Some(SharedReadTarget::Team(team)) => {
            let shared_context = shared_context.ok_or(shared_repository_required_message())?;
            sections.push(ShelfSection {
                title: format!("Shared / {team}"),
                shelves: list_team_shelves(shared_context, &team)?,
            });
        }
        Some(SharedReadTarget::AllTeams) => {
            let shared_context = shared_context.ok_or(shared_repository_required_message())?;
            sections.extend(sections_from_grouped_team_shelves(list_all_team_shelves(
                shared_context,
            )?));
        }
        None => {}
    }

    print_shelf_sections(&sections, "No shelves available.");
    Ok(())
}

fn sections_from_grouped_team_shelves(grouped: Vec<(String, String)>) -> Vec<ShelfSection> {
    let mut sections = Vec::new();
    let mut current_team = None::<String>;
    let mut current_shelves = Vec::new();

    for (team, shelf) in grouped {
        if current_team.as_deref() != Some(team.as_str()) {
            if let Some(team_name) = current_team.take() {
                sections.push(ShelfSection {
                    title: format!("Shared / {team_name}"),
                    shelves: std::mem::take(&mut current_shelves),
                });
            }
            current_team = Some(team);
        }
        current_shelves.push(shelf);
    }

    if let Some(team_name) = current_team {
        sections.push(ShelfSection {
            title: format!("Shared / {team_name}"),
            shelves: current_shelves,
        });
    }

    sections
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
    config: &ShellshelfConfig,
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
    shelf: &str,
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
            shelf,
            keywords,
        )?,
        None => Vec::new(),
    };

    let hidden_local_duplicates =
        hide_local_duplicates(&mut local_commands, shared_sections.as_slice());

    let mut sections = Vec::new();
    if !local_commands.is_empty() {
        sections.push(OutputSection::local(shelf.to_string(), local_commands));
    }
    sections.extend(shared_sections);

    Ok((sections, hidden_local_duplicates))
}

fn load_search_sections_without_active_shelf(
    matches: &clap::ArgMatches,
    config: &ShellshelfConfig,
    shared_context: Option<&SharedStorageContext>,
    keywords: &[String],
) -> Result<(Vec<OutputSection>, usize)> {
    if let Some(team) = matches.get_one::<String>("team") {
        let sections = load_shared_sections_for_team_all_shelves(
            shared_context.ok_or(shared_repository_required_message())?,
            team,
            keywords,
        )?;
        return Ok((sections, 0));
    }

    if matches.get_flag("all-teams") {
        let sections = load_shared_sections_for_all_shelves(
            shared_context.ok_or(shared_repository_required_message())?,
            keywords,
        )?;
        return Ok((sections, 0));
    }

    let plan = resolve_default_read_plan(matches, config, shared_context)?;
    let mut local_sections = if plan.include_local {
        load_local_sections_for_all_shelves(keywords)?
    } else {
        Vec::new()
    };
    let shared_sections = match &plan.shared_target {
        Some(SharedReadTarget::Team(team)) => load_shared_sections_for_team_all_shelves(
            shared_context.ok_or(shared_repository_required_message())?,
            team,
            keywords,
        )?,
        Some(SharedReadTarget::AllTeams) => load_shared_sections_for_all_shelves(
            shared_context.ok_or(shared_repository_required_message())?,
            keywords,
        )?,
        None => Vec::new(),
    };

    let hidden_local_duplicates =
        hide_local_duplicates_in_sections(&mut local_sections, shared_sections.as_slice());
    let mut sections = local_sections;
    sections.extend(shared_sections);
    Ok((sections, hidden_local_duplicates))
}

fn load_shared_sections_for_target(
    shared_context: &SharedStorageContext,
    target: &SharedReadTarget,
    shelf: &str,
    keywords: Option<&[String]>,
) -> Result<Vec<OutputSection>> {
    match target {
        SharedReadTarget::Team(team) => {
            let commands = load_team_commands(shared_context, team, shelf, keywords)?;
            Ok(vec![OutputSection::shared_team(
                team.clone(),
                shelf.to_string(),
                commands
                    .into_iter()
                    .map(OutputEntry::from_owned_command)
                    .collect(),
            )])
        }
        SharedReadTarget::AllTeams => load_shared_sections(shared_context, shelf, keywords),
    }
}

fn load_shared_sections(
    shared_context: &SharedStorageContext,
    shelf: &str,
    keywords: Option<&[String]>,
) -> Result<Vec<OutputSection>> {
    let results = load_all_team_commands(shared_context, shelf, keywords)?;
    let mut sections = Vec::new();
    let mut current_team = None::<String>;
    let mut current_commands = Vec::new();

    for (team, command) in results {
        if current_team.as_deref() != Some(team.as_str()) {
            if let Some(team_name) = current_team.take() {
                sections.push(OutputSection::shared_team(
                    team_name,
                    shelf.to_string(),
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
            shelf.to_string(),
            current_commands,
        ));
    }

    Ok(sections)
}

fn load_local_sections_for_all_shelves(keywords: &[String]) -> Result<Vec<OutputSection>> {
    let mut sections = Vec::new();

    for shelf in list_local_shelves()? {
        let data_file = get_local_data_file_path(&shelf)?;
        let database = CommandDatabase::load_from_file(&data_file)?;
        sections.push(OutputSection::local(
            shelf,
            filter_commands(&database, Some(keywords)),
        ));
    }

    Ok(sections)
}

fn load_shared_sections_for_team_all_shelves(
    shared_context: &SharedStorageContext,
    team: &str,
    keywords: &[String],
) -> Result<Vec<OutputSection>> {
    let mut sections = Vec::new();

    for shelf in list_team_shelves(shared_context, team)? {
        let commands = load_team_commands(shared_context, team, &shelf, Some(keywords))?;
        sections.push(OutputSection::shared_team(
            team.to_string(),
            shelf,
            commands
                .into_iter()
                .map(OutputEntry::from_owned_command)
                .collect(),
        ));
    }

    Ok(sections)
}

fn load_shared_sections_for_all_shelves(
    shared_context: &SharedStorageContext,
    keywords: &[String],
) -> Result<Vec<OutputSection>> {
    let mut sections = Vec::new();

    for (team, shelf) in list_all_team_shelves(shared_context)? {
        let commands = load_team_commands(shared_context, &team, &shelf, Some(keywords))?;
        sections.push(OutputSection::shared_team(
            team,
            shelf,
            commands
                .into_iter()
                .map(OutputEntry::from_owned_command)
                .collect(),
        ));
    }

    Ok(sections)
}

fn hide_local_duplicates(
    local_commands: &mut Vec<OutputEntry>,
    shared_sections: &[OutputSection],
) -> usize {
    let shared_commands = shared_commands(shared_sections);

    if shared_commands.is_empty() {
        return 0;
    }

    let original_len = local_commands.len();
    local_commands.retain(|command| !shared_commands.contains(command.command.as_str()));
    original_len.saturating_sub(local_commands.len())
}

fn hide_local_duplicates_in_sections(
    local_sections: &mut [OutputSection],
    shared_sections: &[OutputSection],
) -> usize {
    let shared_commands = shared_commands(shared_sections);

    if shared_commands.is_empty() {
        return 0;
    }

    let mut hidden = 0;
    for section in local_sections {
        if !matches!(section.source, OutputSectionSource::Local { .. }) {
            continue;
        }

        let original_len = section.entries.len();
        section
            .entries
            .retain(|entry| !shared_commands.contains(entry.command.as_str()));
        hidden += original_len.saturating_sub(section.entries.len());
    }

    hidden
}

fn shared_commands(shared_sections: &[OutputSection]) -> HashSet<&str> {
    shared_sections
        .iter()
        .filter(|section| section.is_shared())
        .flat_map(|section| section.entries.iter().map(|entry| entry.command.as_str()))
        .collect()
}

fn resolve_list_limit(matches: &clap::ArgMatches, config: &ShellshelfConfig) -> Option<usize> {
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

fn empty_message(filtered: bool, shelf: Option<&str>) -> String {
    match (filtered, shelf) {
        (true, Some(shelf)) => format!("No matching commands in shelf '{shelf}'."),
        (false, Some(shelf)) => format!("No commands stored in shelf '{shelf}'."),
        (true, None) => "No matching commands in any shelf.".to_string(),
        (false, None) => "No commands stored in any shelf.".to_string(),
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

fn print_shelf_sections(sections: &[ShelfSection], empty_message: &str) {
    let sections: Vec<&ShelfSection> = sections
        .iter()
        .filter(|section| !section.shelves.is_empty())
        .collect();

    if sections.is_empty() {
        println!("{empty_message}");
        return;
    }

    for (section_index, section) in sections.iter().enumerate() {
        if section_index > 0 {
            println!();
        }

        println!("{}", format_section_header(&section.title));
        println!();

        for (index, shelf) in section.shelves.iter().enumerate() {
            println!("[{}] {}", index + 1, shelf);
        }
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
