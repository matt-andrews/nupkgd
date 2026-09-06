use clap::{Parser, Subcommand};
use std::path::{Path, PathBuf};
use std::process;
use std::time::Duration;
use tokio::net::TcpListener;
use models::app_state;
use notify::{Config, Event, EventKind, RecommendedWatcher, RecursiveMode, Watcher};
use crate::models::app_state::AppState;

pub mod models;
pub mod routes;

#[derive(Parser)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    Start {
        #[arg(long)]
        dir: PathBuf,
        #[arg(long)]
        base_url: Option<String>,
        #[arg(long, default_value = "5555")]
        bind: String,
    }
}

#[tokio::main]
async fn main() -> process::ExitCode {
    match run().await {
        Ok(exit) => process::ExitCode::from(exit as u8),
        Err(e) => {
            eprintln!("{e:#}");
            process::ExitCode::from(1)
        },
    }
}

async fn run() -> anyhow::Result<u8>{
    let args = Cli::parse();
    match args.command {
        Commands::Start {
            dir,
            base_url,
            bind,
        } => {
            let state = app_state::AppState::from_dir(&dir, base_url)?;

            // --- watcher
            let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel::<notify::Result<Event>>();
            let mut _watcher = RecommendedWatcher::new(
                move |res| { let _ = tx.send(res); },
                Config::default(),
            )?;
            _watcher.watch(&dir, RecursiveMode::NonRecursive)?;

            let watch_state = state.clone();
            tokio::spawn(async move {
                while let Some(res) = rx.recv().await {
                    let ev = match res {
                        Ok(ev) => ev,
                        Err(e) => { eprintln!("watch error: {e}"); continue; }
                    };
                    //if !matches!(ev.kind, EventKind::Create(_)) { continue; }

                    for path in ev.paths.into_iter().filter(|p| is_nupkg(p)) {
                        let st = watch_state.clone();
                        tokio::spawn(async move {
                            if let Err(e) = add_with_retry(&st, &path, ev.kind).await {
                                eprintln!("failed to add {}: {e}", path.display());
                            }
                        });
                    }
                }
            });

            // --- server
            let listener = TcpListener::bind(format!("0.0.0.0:{}", bind)).await?;
            println!("listening on :{}", bind);
            axum::serve(listener, routes::router(state)).await?;

            Ok(0u8)
        }
    }
}

fn is_nupkg(p: &Path) -> bool {
    p.extension()
        .and_then(|e| e.to_str())
        .is_some_and(|e| e.eq_ignore_ascii_case("nupkg"))
}

/// Windows ERROR_SHARING_VIOLATION anywhere in the error chain
fn is_sharing_violation(e: &anyhow::Error) -> bool {
    e.chain()
        .filter_map(|c| c.downcast_ref::<std::io::Error>())
        .any(|io| io.raw_os_error() == Some(32))
}

async fn add_with_retry(state: &AppState, path: &PathBuf, kind: EventKind) -> anyhow::Result<()> {
    let mut delay = Duration::from_millis(100);
    let max_attempts = 10;
    for attempt in 1..=max_attempts {
        match kind {
            EventKind::Remove(_) => {
                match state.remove_file(path).await {
                    Ok(()) => return Ok(()),
                    Err(e) if is_sharing_violation(&e) && attempt < max_attempts => {
                        tokio::time::sleep(delay).await;
                        delay = (delay * 2).min(Duration::from_secs(2));
                    }
                    Err(e) => return Err(e),
                }
            },
            _ => {
                match state.add_file(path).await {
                    Ok(()) => return Ok(()),
                    Err(e) if is_sharing_violation(&e) && attempt < max_attempts => {
                        tokio::time::sleep(delay).await;
                        delay = (delay * 2).min(Duration::from_secs(2));
                    }
                    Err(e) => return Err(e),
                }
            },
        }

    }
    unreachable!()
}