use log::{error, info, warn};
use simple_logger::SimpleLogger;
use std::{collections::HashMap, str::FromStr, sync::Arc, sync::Mutex};

use hickory_server::{
    authority::MessageResponseBuilder,
    proto::op::{Header, MessageType, OpCode, ResponseCode},
    server::{Request, RequestHandler, ResponseHandler, ResponseInfo},
};
use hickory_resolver::{
    config::{ResolverConfig, ResolverOpts},
    AsyncResolver,
};
use hickory_proto::rr::{rdata::TXT, LowerName, Name, RData, Record};
use crate::options::Options;

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error("Invalid OpCode {0:?}")]
    InvalidOpCode(OpCode),
    #[error("Invalid MessageType {0:?}")]
    InvalidMessageType(MessageType),
    #[error("IO error: {0:?}")]
    Io(#[from] std::io::Error),
    #[error("Resolver error: {0:?}")]
    ResolveError(#[from] hickory_resolver::error::ResolveError),
}

pub struct Handler {
    pub root_zone: LowerName,
    pub data: Arc<Mutex<HashMap<String, Vec<String>>>>,
    pub resolver: Arc<
        AsyncResolver<
            hickory_resolver::name_server::GenericConnector<
                hickory_resolver::name_server::TokioRuntimeProvider,
            >,
        >,
    >,
}

impl Handler {
    pub fn from_options(options: &Options) -> Self {
        SimpleLogger::new().init().unwrap();
        info!("Initializing DNS Server...");

        let domain = &options.domain;

        let mut resolver_config = ResolverConfig::new();
        resolver_config.add_name_server(hickory_resolver::config::NameServerConfig::new(
            std::net::SocketAddr::from_str("9.9.9.9:53").unwrap(),
            hickory_resolver::config::Protocol::Udp,
        ));

        let resolver_opts = ResolverOpts::default();
        let resolver = AsyncResolver::tokio(resolver_config, resolver_opts);

        Handler {
            root_zone: LowerName::from(Name::from_str(domain).unwrap()),
            data: Arc::new(Mutex::new(HashMap::new())),
            resolver: Arc::new(resolver),
        }
    }

    async fn do_handle_request<R: ResponseHandler>(
        &self,
        request: &Request,
        response: R,
    ) -> Result<ResponseInfo, Error> {
        if request.op_code() != OpCode::Query {
            error!("Invalid OpCode received: {:?}", request.op_code());
            return Err(Error::InvalidOpCode(request.op_code()));
        }

        if request.message_type() != MessageType::Query {
            error!("Invalid MessageType received: {:?}", request.message_type());
            return Err(Error::InvalidMessageType(request.message_type()));
        }

        let query_name = request.query().name().to_string();
        info!(
            "Received request: type={:?}, domain={}",
            request.query().query_type(),
            query_name
        );

        match query_name.as_str() {
            name if name.starts_with(&self.root_zone.to_string()) => {
                self.do_handle_request_test(request, response).await
            }
            name if name.starts_with("upload.") => {
                self.do_handle_request_upload(request, response).await
            }
            name if name.starts_with("download.") => {
                self.do_handle_request_download(request, response).await
            }
            name if name.starts_with("close.") => {
                self.do_handle_request_close(request, response).await
            }
            _ => {
                info!("Forwarding request to external resolver: {}", query_name);
                self.forward_request(request, response).await
            }
        }
    }

    async fn do_handle_request_test<R: ResponseHandler>(
        &self,
        request: &Request,
        mut responder: R,
    ) -> Result<ResponseInfo, Error> {
        info!("Handling test request for domain: {}", request.query().name());
        let builder = MessageResponseBuilder::from_message_request(request);

        let mut header = Header::response_from_request(request.header());
        header.set_authoritative(true);

        let rdata = RData::A("82.165.140.49".parse().unwrap());
        let records = vec![Record::from_rdata(request.query().name().into(), 60, rdata)];

        let response = builder.build(header, records.iter(), &[], &[], &[]);
        info!("Test request successfully handled.");
        Ok(responder.send_response(response).await?)
    }

    async fn do_handle_request_upload<R: ResponseHandler>(
        &self,
        request: &Request,
        mut responder: R,
    ) -> Result<ResponseInfo, Error> {
        info!("Handling upload request: {}", request.query().name());
        let builder = MessageResponseBuilder::from_message_request(request);
        let mut header = Header::response_from_request(request.header());
        header.set_authoritative(true);

        let query_name = request.query().name().to_string();
        let parts: Vec<&str> = query_name.split('.').collect();
        let mut message = String::from("OK");

        if parts.len() < 5 {
            error!("Invalid upload request format: {}", request.query().name());
            message = "ERROR: Invalid request format".to_string();
        } else {
            let uid = parts[4].to_string();
            let maxseq: usize = parts[3].parse().unwrap_or(0);
            let seq: usize = parts[2].parse().unwrap_or(0);
            let data_fragment = parts[1].to_string();

            match self.data.lock() {
                Ok(mut data) => {
                    let fragments = data.entry(uid.clone()).or_insert_with(|| vec![String::new(); maxseq]);
                    if seq >= maxseq {
                        message = "ERROR: Invalid sequence".to_string();
                        error!("Failed to upload fragment: UID={} Seq={}", uid, seq);
                    } else if !fragments[seq].is_empty() {
                        message = "ERROR: Duplicate sequence".to_string();
                        info!("Duplicate fragment: UID={} Seq={}", uid, seq);
                    } else {
                        fragments[seq] = data_fragment;
                        info!("Uploaded fragment: UID={} Seq={}/{}", uid, seq + 1, maxseq);
                    }
                }
                Err(e) => {
                    error!("Failed to acquire lock on data: {}", e);
                    message = "ERROR: Failed to acquire lock".to_string();
                }
            }
        }

        let rdata = RData::TXT(TXT::new(vec![message]));
        let records = vec![Record::from_rdata(request.query().name().into(), 60, rdata)];

        let response = builder.build(header, records.iter(), &[], &[], &[]);
        Ok(responder.send_response(response).await?)
    }

    async fn do_handle_request_download<R: ResponseHandler>(
        &self,
        request: &Request,
        mut responder: R,
    ) -> Result<ResponseInfo, Error> {
        info!("Handling download request: {}", request.query().name());
        let builder = MessageResponseBuilder::from_message_request(request);
        let mut header = Header::response_from_request(request.header());
        header.set_authoritative(true);

        let query_name = request.query().name().to_string();
        let parts: Vec<&str> = query_name.split('.').collect();
        let mut message = String::from("OK");

        if parts.len() < 3 {
            error!("Invalid download request format: {}", request.query().name());
            message = "ERROR: Invalid request format".to_string();
        } else {
            let code = parts[1].to_string();
            let seq = parts.get(2).and_then(|s| s.parse::<usize>().ok());

            match self.data.lock() {
                Ok(data) => {
                    if let Some(fragments) = data.get(&code) {
                        message = match seq {
                            Some(seq) if seq < fragments.len() => {
                                info!("Downloaded fragment: UID={} Seq={}", code, seq);
                                fragments[seq].clone()
                            }
                            None => fragments.len().to_string(),
                            _ => "ERROR: Invalid sequence number".to_string(),
                        };
                    } else {
                        message = "EOF".to_string();
                        warn!("Data not found for UID={}", code);
                    }
                }
                Err(e) => {
                    error!("Failed to acquire lock on data: {}", e);
                    message = "ERROR: Failed to acquire lock".to_string();
                }
            }
        }

        let rdata = RData::TXT(TXT::new(vec![message]));
        let records = vec![Record::from_rdata(request.query().name().into(), 60, rdata)];

        let response = builder.build(header, records.iter(), &[], &[], &[]);
        Ok(responder.send_response(response).await?)
    }

    async fn do_handle_request_close<R: ResponseHandler>(
        &self,
        request: &Request,
        mut responder: R,
    ) -> Result<ResponseInfo, Error> {
        info!("Handling close request: {}", request.query().name());
        let builder = MessageResponseBuilder::from_message_request(request);
        let mut header = Header::response_from_request(request.header());
        header.set_authoritative(true);

        let query_name = request.query().name().to_string();
        let parts: Vec<&str> = query_name.split('.').collect();
        let uid = parts.get(1).unwrap_or(&"").to_string();
        let mut message = "OK".to_string();

        if let Ok(mut data) = self.data.lock() {
            if data.remove(&uid).is_some() {
                info!("Session closed for UID={}", uid);
            } else {
                warn!("Close request for non-existent UID={}", uid);
                message = "ERROR: UID not found".to_string();
            }
        } else {
            error!("Failed to acquire lock for closing UID={}", uid);
            message = "ERROR: Failed to acquire lock".to_string();
        }

        let rdata = RData::TXT(TXT::new(vec![message]));
        let records = vec![Record::from_rdata(request.query().name().into(), 60, rdata)];

        let response = builder.build(header, records.iter(), &[], &[], &[]);
        Ok(responder.send_response(response).await?)
    }

    async fn forward_request<R: ResponseHandler>(
        &self,
        request: &Request,
        mut responder: R,
    ) -> Result<ResponseInfo, Error> {
        let name = request.query().name().to_string();
        info!("Forwarding DNS request to resolver: {}", name);

        let records = self
            .resolver
            .lookup(name.clone(), request.query().query_type())
            .await
            .map_err(|e| {
                error!("DNS resolution error for {}: {}", name, e);
                e
            })?;

        let builder = MessageResponseBuilder::from_message_request(request);
        let mut header = Header::response_from_request(request.header());
        header.set_authoritative(false);

        let response_records: Vec<Record> = records
            .into_iter()
            .map(|r| Record::from_rdata(request.query().name().into(), 300, r))
            .collect();

        let response = builder.build(header, response_records.iter(), &[], &[], &[]);
        Ok(responder.send_response(response).await?)
    }
}

#[async_trait::async_trait]
impl RequestHandler for Handler {
    async fn handle_request<R: ResponseHandler>(
        &self,
        request: &Request,
        response: R,
    ) -> ResponseInfo {
        match self.do_handle_request(request, response).await {
            Ok(info) => info,
            Err(e) => {
                error!("Error handling request: {}", e);
                let mut header = Header::new();
                header.set_response_code(ResponseCode::ServFail);
                header.into()
            }
        }
    }
}
