use clap::{Parser, Subcommand};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

use rocket_manifest::{api, db, mcp};

#[derive(Parser)]
#[command(name = "rocket-manifest")]
#[command(about = "Living feature documentation for AI-assisted development")]
struct Cli {
    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Subcommand)]
enum Commands {
    /// Start the RocketManifest server
    Serve {
        /// Port for HTTP API
        #[arg(short, long, default_value = "3000")]
        port: u16,

        /// Run as daemon
        #[arg(short, long)]
        daemon: bool,
    },
    /// Start MCP server via stdio (for Claude Code integration)
    Mcp,
    /// Check server status
    Status,
    /// Stop the daemon
    Stop,
}

/// Initialize tracing with output to stderr (for MCP mode) or stdout
fn init_tracing(use_stderr: bool) {
    let filter = tracing_subscriber::EnvFilter::new(
        std::env::var("RUST_LOG")
            .unwrap_or_else(|_| "rocket_manifest=debug,tower_http=debug".into()),
    );

    if use_stderr {
        // MCP mode: log to stderr so stdout is clean for protocol
        tracing_subscriber::registry()
            .with(filter)
            .with(tracing_subscriber::fmt::layer().with_writer(std::io::stderr))
            .init();
    } else {
        tracing_subscriber::registry()
            .with(filter)
            .with(tracing_subscriber::fmt::layer())
            .init();
    }
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();

    // MCP mode needs stderr for logging since stdout is the protocol channel
    let use_stderr = matches!(cli.command, Some(Commands::Mcp));
    init_tracing(use_stderr);

    match cli.command {
        Some(Commands::Serve { port, daemon: _ }) => {
            tracing::info!("Starting RocketManifest server on port {}", port);

            let db = db::Database::open_default()?;
            db.migrate()?;

            let app = api::create_router(db);

            let listener = tokio::net::TcpListener::bind(format!("127.0.0.1:{}", port)).await?;
            tracing::info!(
                "RocketManifest server listening on http://127.0.0.1:{}",
                port
            );

            axum::serve(listener, app).await?;
        }
        Some(Commands::Mcp) => {
            let db = db::Database::open_default()?;
            db.migrate()?;

            mcp::run_stdio_server(db).await?;
        }
        Some(Commands::Status) => {
            println!("Checking RocketManifest server status...");
            // TODO: Check if server is running
        }
        Some(Commands::Stop) => {
            println!("Stopping RocketManifest server...");
            // TODO: Stop daemon
        }
        None => {
            // Default: start server
            tracing::info!("Starting RocketManifest server on port 3000");

            let db = db::Database::open_default()?;
            db.migrate()?;

            let app = api::create_router(db);

            let listener = tokio::net::TcpListener::bind("127.0.0.1:3000").await?;
            tracing::info!("RocketManifest server listening on http://127.0.0.1:3000");

            axum::serve(listener, app).await?;
        }
    }

    Ok(())
}
