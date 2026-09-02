use std::{fs::File, io::BufReader, path::PathBuf, sync::Arc};

use activity_tracker::{
    ActivityRepository, MongoActivityRepository,
    interviewstack::{dry_run_report, import_rows, parse_ndjson},
};
use clap::Parser;

#[derive(Parser, Debug)]
#[command(about = "Validate and idempotently import InterviewStack history NDJSON")]
struct Args {
    #[arg(long)]
    input: PathBuf,
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
    let file = File::open(&args.input)?;
    let (rows, failures) = parse_ndjson(BufReader::new(file));
    let total_lines = rows.len() + failures.len();
    let report = if args.dry_run {
        dry_run_report(&rows, failures, total_lines)
    } else {
        let uri = args
            .mongodb_uri
            .ok_or("--mongodb-uri or MONGODB_URI is required unless --dry-run is used")?;
        let repository = MongoActivityRepository::connect(&uri, &args.mongodb_database).await?;
        repository.ensure_indexes().await?;
        import_rows(&rows, failures, total_lines, Arc::new(repository)).await?
    };
    println!("{}", serde_json::to_string_pretty(&report)?);
    if report.failed > 0 {
        std::process::exit(2);
    }
    Ok(())
}
