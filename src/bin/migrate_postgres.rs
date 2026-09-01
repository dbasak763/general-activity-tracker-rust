use std::sync::Arc;

use activity_tracker::{
    ActivityRepository, MongoActivityRepository,
    migration::{load_postgres_attempts, migrate},
};
use clap::Parser;
use sqlx::postgres::PgPoolOptions;

#[derive(Parser, Debug)]
#[command(
    about = "Idempotently migrate interview_attempts from PostgreSQL into MongoDB activities"
)]
struct Args {
    #[arg(long, env = "DATABASE_URL")]
    postgres_url: String,
    #[arg(long, env = "MONGODB_URI")]
    mongodb_uri: Option<String>,
    #[arg(long, env = "MONGODB_DATABASE", default_value = "activity_tracker")]
    mongodb_database: String,
    #[arg(long)]
    dry_run: bool,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    dotenvy::dotenv().ok();
    let args = Args::parse();
    let pool = PgPoolOptions::new()
        .max_connections(4)
        .connect(&args.postgres_url)
        .await?;
    let rows = load_postgres_attempts(&pool).await?;
    if args.dry_run {
        let report = activity_tracker::migration::MigrationReport {
            source_count: rows.len(),
            mapped_count: rows
                .iter()
                .cloned()
                .map(activity_tracker::migration::map_postgres_attempt)
                .collect::<Result<Vec<_>, _>>()?
                .len(),
            dry_run: true,
            max_legacy_id: rows.iter().map(|row| row.id).max(),
            ..Default::default()
        };
        println!("{}", serde_json::to_string_pretty(&report)?);
        return Ok(());
    }
    let uri = args
        .mongodb_uri
        .ok_or("--mongodb-uri or MONGODB_URI is required unless --dry-run is used")?;
    let repository = MongoActivityRepository::connect(&uri, &args.mongodb_database).await?;
    repository.ensure_indexes().await?;
    let report = migrate(rows, Arc::new(repository), false).await?;
    println!("{}", serde_json::to_string_pretty(&report)?);
    Ok(())
}
