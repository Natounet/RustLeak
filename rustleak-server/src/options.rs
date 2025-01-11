use clap::Parser;
use std::net::SocketAddr;

#[derive(Parser, Clone, Debug)]
pub struct Options {
    /// UDP socket to listen on.
    #[clap(long, short, default_value = "0.0.0.0:1053")]
    pub udp: Vec<SocketAddr>,

    /// Domain name
    #[clap(long, short)]
    pub domain: String,
}
