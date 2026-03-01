mod analyze;
mod client;
mod error;
mod tui;
mod types;

use clap::{Parser, Subcommand};
use colored::Colorize;
use crossterm::event::{self, Event, KeyCode, KeyEventKind, KeyModifiers};
use futures_util::StreamExt;
use std::env;
use std::fs;
use std::io::{self, ErrorKind};
use std::path::{Path, PathBuf};
use std::process;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::mpsc;
use tokio::sync::Semaphore;

use crate::analyze::synthesize_report;
use crate::client::TinyFishClient;
use crate::tui::AgentStatus;
use crate::types::{AutomationRequest, RunStatus, SseEvent};

#[derive(Parser)]
#[command(version, about = "CLI for the TinyFish web automation API")]
struct Cli {
    /// API key for TinyFish
    #[arg(short, long, env = "TINYFISH_API_KEY")]
    api_key: String,

    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Run an automation task
    Run {
        /// The URL to start the automation on
        url: String,

        /// The goal of the automation task
        goal: String,
    },

    /// Run multiple automation tasks concurrently in the swarm TUI
    Swarm {
        /// File containing URLs (one per line)
        #[arg(short, long, value_name = "FILE")]
        file: PathBuf,

        /// Shared goal for every URL in the swarm
        goal: String,

        /// Synthesize successful swarm outputs into a markdown report
        #[arg(short, long)]
        analyze: bool,
    },
}

#[derive(Debug, Clone)]
enum AgentUpdate {
    Log(usize, String),
    StatusChange(usize, AgentStatus),
}

#[tokio::main]
async fn main() {
    dotenvy::dotenv().ok();
    let cli = Cli::parse();

    match cli.command {
        Commands::Run { url, goal } => {
            if let Err(e) = run_automation(cli.api_key, url, goal).await {
                eprintln!("{}", format!("Error: {}", e).red());
                process::exit(1);
            }
        }
        Commands::Swarm {
            file,
            goal,
            analyze,
        } => {
            if let Err(e) = run_swarm(cli.api_key, file, goal, analyze).await {
                eprintln!("{}", format!("Error: {}", e).red());
                process::exit(1);
            }
        }
    }
}

async fn run_automation(
    api_key: String,
    url: String,
    goal: String,
) -> Result<(), crate::error::KrillError> {
    let client = TinyFishClient::new(api_key);

    let payload = AutomationRequest {
        url,
        goal,
        browser_profile: None,
        proxy_config: None,
        api_integration: None,
        feature_flags: None,
    };

    let stream = client.run_sse(payload).await?;
    tokio::pin!(stream);

    while let Some(result) = stream.next().await {
        match result {
            Ok(event) => {
                match event {
                    SseEvent::Progress {
                        purpose, message, ..
                    } => {
                        if let Some(log_line) = format_progress_line(purpose, message) {
                            eprintln!("{}", log_line.dimmed());
                        }
                    }
                    SseEvent::Complete { result_json, .. } => {
                        if let Some(json) = result_json {
                            match serde_json::to_string_pretty(&json) {
                                Ok(pretty) => println!("{}", pretty),
                                Err(e) => {
                                    eprintln!("{}", format!("Error formatting JSON: {}", e).red());
                                    process::exit(1);
                                }
                            }
                        } else {
                            println!("{{}}");
                        }
                    }
                    _ => {
                        // Ignore Heartbeat, Started, StreamingUrl, etc.
                    }
                }
            }
            Err(e) => {
                eprintln!("{}", format!("Stream error: {}", e).red());
                process::exit(1);
            }
        }
    }

    Ok(())
}

async fn run_swarm(
    api_key: String,
    file: PathBuf,
    goal: String,
    analyze: bool,
) -> Result<(), crate::error::KrillError> {
    let urls = read_urls_from_file(&file)?;
    let agents = urls
        .iter()
        .map(|url| tui::AgentState::new(url.clone(), goal.clone()))
        .collect();
    let mut app = tui::App::new(agents);

    let (sender, mut receiver) = mpsc::channel::<AgentUpdate>(1024);
    let mut workers = Vec::with_capacity(urls.len());
    let semaphore = Arc::new(Semaphore::new(10));

    for (agent_idx, url) in urls.into_iter().enumerate() {
        let worker_sender = sender.clone();
        let worker_api_key = api_key.clone();
        let worker_goal = goal.clone();
        let worker_semaphore = Arc::clone(&semaphore);
        workers.push(tokio::spawn(async move {
            run_swarm_worker(
                agent_idx,
                worker_api_key,
                url,
                worker_goal,
                worker_sender,
                worker_semaphore,
            )
            .await;
        }));
    }
    drop(sender);

    run_swarm_ui(&mut app, &mut receiver)?;

    for worker in workers {
        worker.abort();
    }

    if analyze {
        let openai_api_key = match env::var("OPENAI_API_KEY") {
            Ok(value) if !value.trim().is_empty() => value,
            _ => {
                eprintln!(
                    "{}",
                    "Error: Missing OPENAI_API_KEY environment variable".red()
                );
                process::exit(1);
            }
        };

        let successful_agents: Vec<_> = app
            .agents
            .iter()
            .filter(|agent| matches!(agent.status, AgentStatus::Success))
            .collect();

        if successful_agents.is_empty() {
            eprintln!(
                "{}",
                "Error: --analyze requested but no successful agent data is available.".red()
            );
            process::exit(1);
        }

        let aggregated_data = successful_agents
            .iter()
            .map(|agent| {
                let logs = agent
                    .logs
                    .iter()
                    .map(String::as_str)
                    .collect::<Vec<_>>()
                    .join("\n");

                format!(
                    "URL: {}\nGoal: {}\nStatus: {}\nLogs:\n{}",
                    agent.url,
                    agent.goal,
                    agent.status.label(),
                    logs
                )
            })
            .collect::<Vec<_>>()
            .join("\n\n---\n\n");

        eprintln!("{}", " Synthesizing intelligence report with OpenAI...".cyan());
        let report = synthesize_report(&aggregated_data, &openai_api_key)
            .await
            .map_err(|error| {
                crate::error::KrillError::Io(io::Error::new(
                    ErrorKind::Other,
                    format!("OpenAI synthesis failed: {}", error),
                ))
            })?;

        let report_file = "market_analysis_report.md";
        tokio::fs::write(report_file, &report).await?;
        println!("\n");
        termimad::print_text(&report);
        println!("\n");
        let report_path = env::current_dir()?.join(report_file);
        eprintln!(
            "{}",
            format!(
                " Intelligence report saved to {}",
                report_path.display()
            )
            .green()
        );
    }

    Ok(())
}

async fn run_swarm_worker(
    agent_idx: usize,
    api_key: String,
    url: String,
    goal: String,
    sender: mpsc::Sender<AgentUpdate>,
    semaphore: Arc<Semaphore>,
) {
    // Stay in Pending until we acquire a permit from the connection pool
    let _permit = semaphore.acquire().await.unwrap();

    if !send_update(
        &sender,
        AgentUpdate::StatusChange(agent_idx, AgentStatus::Running),
    )
    .await
    {
        return;
    }

    let payload = AutomationRequest {
        url,
        goal,
        browser_profile: None,
        proxy_config: None,
        api_integration: None,
        feature_flags: None,
    };

    let client = TinyFishClient::new(api_key);
    let stream = match client.run_sse(payload).await {
        Ok(stream) => stream,
        Err(error) => {
            let message = format!("Failed to start stream: {}", error);
            let _ = send_update(&sender, AgentUpdate::Log(agent_idx, message.clone())).await;
            let _ = send_update(
                &sender,
                AgentUpdate::StatusChange(agent_idx, AgentStatus::Error(message)),
            )
            .await;
            return;
        }
    };

    tokio::pin!(stream);
    let mut has_terminal_status = false;

    while let Some(event_result) = stream.next().await {
        match event_result {
            Ok(event) => match event {
                SseEvent::Started { run_id, .. } => {
                    if !send_update(
                        &sender,
                        AgentUpdate::Log(agent_idx, format!("run_id: {}", run_id)),
                    )
                    .await
                    {
                        return;
                    }
                }
                SseEvent::StreamingUrl { streaming_url, .. } => {
                    if !send_update(
                        &sender,
                        AgentUpdate::Log(agent_idx, format!("stream: {}", streaming_url)),
                    )
                    .await
                    {
                        return;
                    }
                }
                SseEvent::Progress {
                    purpose, message, ..
                } => {
                    if let Some(log_line) = format_progress_line(purpose, message) {
                        if !send_update(&sender, AgentUpdate::Log(agent_idx, log_line)).await {
                            return;
                        }
                    }
                }
                SseEvent::Complete {
                    status,
                    result_json,
                    ..
                } => {
                    if let Some(result) = result_json {
                        match serde_json::to_string_pretty(&result) {
                            Ok(pretty) => {
                                for line in pretty.lines() {
                                    if !send_update(
                                        &sender,
                                        AgentUpdate::Log(agent_idx, line.to_owned()),
                                    )
                                    .await
                                    {
                                        return;
                                    }
                                }
                            }
                            Err(error) => {
                                if !send_update(
                                    &sender,
                                    AgentUpdate::Log(
                                        agent_idx,
                                        format!("Error formatting JSON: {}", error),
                                    ),
                                )
                                .await
                                {
                                    return;
                                }
                            }
                        }
                    }

                    match status {
                        RunStatus::Completed => {
                            has_terminal_status = true;
                            if !send_update(
                                &sender,
                                AgentUpdate::StatusChange(agent_idx, AgentStatus::Success),
                            )
                            .await
                            {
                                return;
                            }
                        }
                        RunStatus::Failed | RunStatus::Cancelled => {
                            has_terminal_status = true;
                            if !send_update(
                                &sender,
                                AgentUpdate::StatusChange(
                                    agent_idx,
                                    AgentStatus::Error(format!("Run ended with {:?}", status)),
                                ),
                            )
                            .await
                            {
                                return;
                            }
                        }
                        RunStatus::Running | RunStatus::Queued => {
                            if !send_update(
                                &sender,
                                AgentUpdate::StatusChange(agent_idx, AgentStatus::Running),
                            )
                            .await
                            {
                                return;
                            }
                        }
                    }
                }
                SseEvent::Heartbeat { .. } => {}
            },
            Err(error) => {
                let message = format!("Stream error: {}", error);
                let _ = send_update(&sender, AgentUpdate::Log(agent_idx, message.clone())).await;
                let _ = send_update(
                    &sender,
                    AgentUpdate::StatusChange(agent_idx, AgentStatus::Error(message)),
                )
                .await;
                return;
            }
        }
    }

    if !has_terminal_status {
        let _ = send_update(
            &sender,
            AgentUpdate::StatusChange(
                agent_idx,
                AgentStatus::Error("Stream closed without terminal status".to_owned()),
            ),
        )
        .await;
    }
}

async fn send_update(sender: &mpsc::Sender<AgentUpdate>, update: AgentUpdate) -> bool {
    sender.send(update).await.is_ok()
}

fn run_swarm_ui(
    app: &mut tui::App,
    receiver: &mut mpsc::Receiver<AgentUpdate>,
) -> Result<(), crate::error::KrillError> {
    let terminal = tui::setup_terminal()?;
    let mut terminal_guard = TerminalGuard::new(terminal);

    loop {
        if event::poll(Duration::from_millis(16))? {
            if let Event::Key(key) = event::read()? {
                if key.kind == KeyEventKind::Press {
                    if key.code == KeyCode::Char('c')
                        && key.modifiers.contains(KeyModifiers::CONTROL)
                    {
                        app.should_quit = true;
                    }
                    match key.code {
                        KeyCode::Char('q') | KeyCode::Esc => app.should_quit = true,
                        KeyCode::Tab | KeyCode::Right | KeyCode::Down => {
                            if !app.agents.is_empty() {
                                app.selected = (app.selected + 1) % app.agents.len();
                            }
                        }
                        KeyCode::BackTab | KeyCode::Left | KeyCode::Up => {
                            if !app.agents.is_empty() {
                                app.selected =
                                    app.selected.checked_sub(1).unwrap_or(app.agents.len() - 1);
                            }
                        }
                        _ => {}
                    }
                }
            }
        }

        while let Ok(update) = receiver.try_recv() {
            apply_agent_update(app, update);
        }

        terminal_guard
            .terminal_mut()
            .draw(|f| tui::draw_ui(f, app))?;

        if app.should_quit {
            break;
        }
    }

    Ok(())
}

fn apply_agent_update(app: &mut tui::App, update: AgentUpdate) {
    match update {
        AgentUpdate::Log(agent_idx, message) => {
            if let Some(agent) = app.agents.get_mut(agent_idx) {
                agent.push_log(message);
                agent.progress = (agent.progress + 5).min(95);
            }
        }
        AgentUpdate::StatusChange(agent_idx, status) => {
            if let Some(agent) = app.agents.get_mut(agent_idx) {
                if matches!(status, AgentStatus::Success) {
                    agent.progress = 100;
                }
                agent.status = status;
            }
        }
    }
}

fn read_urls_from_file(file: &Path) -> Result<Vec<String>, crate::error::KrillError> {
    let contents = fs::read_to_string(file)?;
    let urls: Vec<String> = contents
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .map(ToOwned::to_owned)
        .collect();

    if urls.is_empty() {
        return Err(crate::error::KrillError::Io(io::Error::new(
            ErrorKind::InvalidInput,
            format!("No URLs found in {}", file.display()),
        )));
    }

    Ok(urls)
}

fn format_progress_line(purpose: Option<String>, message: Option<String>) -> Option<String> {
    let purpose = purpose.unwrap_or_default();
    let message = message.unwrap_or_default();

    if purpose.is_empty() && message.is_empty() {
        None
    } else if purpose.is_empty() {
        Some(message)
    } else if message.is_empty() {
        Some(purpose)
    } else {
        Some(format!("[{}] {}", purpose, message))
    }
}

struct TerminalGuard {
    terminal: Option<tui::Tui>,
}

impl TerminalGuard {
    fn new(terminal: tui::Tui) -> Self {
        Self {
            terminal: Some(terminal),
        }
    }

    fn terminal_mut(&mut self) -> &mut tui::Tui {
        self.terminal
            .as_mut()
            .expect("terminal must be available before draw")
    }
}

impl Drop for TerminalGuard {
    fn drop(&mut self) {
        if let Some(terminal) = self.terminal.as_mut() {
            let _ = tui::restore_terminal(terminal);
        }
    }
}
