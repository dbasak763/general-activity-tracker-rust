use std::sync::Arc;

use activity_tracker::{ActivityRepository, AppState, Config, MongoActivityRepository, app};
use tokio::net::TcpListener;
use tracing_subscriber::{EnvFilter, fmt, prelude::*};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let config = Config::from_env()?;
    init_tracing(config.json_logs);
    let repository =
        MongoActivityRepository::connect(&config.mongodb_uri, &config.mongodb_database).await?;
    repository.ping().await?;
    repository.ensure_indexes().await?;
    let router = app(
        AppState {
            repository: Arc::new(repository),
            database_name: config.mongodb_database.clone(),
        },
        &config.cors_allowed_origins,
    )?;
    let listener = TcpListener::bind(config.socket_addr()?).await?;
    tracing::info!(address = %listener.local_addr()?, "activity tracker listening");
    axum::serve(listener, router)
        .with_graceful_shutdown(shutdown_signal())
        .await?;
    Ok(())
}

fn init_tracing(json: bool) {
    let filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new("activity_tracker=info,tower_http=info"));
    if json {
        tracing_subscriber::registry()
            .with(filter)
            .with(fmt::layer().json())
            .init();
    } else {
        tracing_subscriber::registry()
            .with(filter)
            .with(fmt::layer())
            .init();
    }
}

async fn shutdown_signal() {
    let ctrl_c = async {
        tokio::signal::ctrl_c()
            .await
            .expect("failed to install Ctrl+C handler");
    };
    #[cfg(unix)]
    let terminate = async {
        tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
            .expect("failed to install signal handler")
            .recv()
            .await;
    };
    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();
    tokio::select! { () = ctrl_c => {}, () = terminate => {} }
}
