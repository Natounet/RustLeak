use crate::options::Options;
use base32::Alphabet;
use rustleak_lib::utils::{self, decode_base32};

use hickory_server::{
    authority::MessageResponseBuilder,
    proto::op::{Header, MessageType, OpCode, ResponseCode},
    server::{Request, RequestHandler, ResponseHandler, ResponseInfo},
};

use hickory_resolver::{
    config::{ResolverConfig, ResolverOpts},
    AsyncResolver,
};

use std::io::{Read, Write};

use hickory_proto::rr::{rdata::TXT, LowerName, Name, RData, Record};

use std::{str::FromStr, sync::Arc};

use std::{collections::HashMap, net::TcpStream, sync::Mutex};

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error("Invalid OpCode {0:}")]
    InvalidOpCode(OpCode),
    #[error("Invalid MessageType {0:}")]
    InvalidMessageType(MessageType),
    #[error("Invalid Zone {0:}")]
    InvalidZone(LowerName),
    #[error("IO error: {0:}")]
    Io(#[from] std::io::Error),
    #[error("Resolver error: {0:}")]
    ResolverError(#[from] hickory_resolver::error::ResolveError),
}

/// DNS Request Handler
#[derive(Clone, Debug)]
pub struct Handler {
    pub root_zone: LowerName,
    pub test_zone: LowerName,



    // Hashmap of IDs -> base32 encoded data fragments
    pub data: Arc<Mutex<HashMap<String, Vec<String>>>>,

    // Resolver for forwarding DNS requests
    pub resolver: Arc<
        AsyncResolver<
            hickory_resolver::name_server::GenericConnector<
                hickory_resolver::name_server::TokioRuntimeProvider,
            >,
        >,
    >,
}

impl Handler {
    /// Create new handler from command-line options.
    pub fn from_options(_options: &Options) -> Self {
        let domain = &_options.domain;

        // Create a resolver configuration pointing to 9.9.9.9
        let mut resolver_config = ResolverConfig::new();
        resolver_config.add_name_server(hickory_resolver::config::NameServerConfig::new(
            std::net::SocketAddr::from_str("9.9.9.9:53").unwrap(),
            hickory_resolver::config::Protocol::Udp,
        ));
        let resolver_opts = ResolverOpts::default();

        // Create an async resolver
        let resolver = AsyncResolver::tokio(resolver_config, resolver_opts);

        Handler {
            // Nom de domaine
            root_zone: LowerName::from(Name::from_str(domain).unwrap()),
            // Route de test pour le client
            test_zone: LowerName::from(
                Name::from_str(format!("test.{}", domain).as_str()).unwrap(),
            ),

            // Initialisation de la hashmap pour les fragments de réponse
            data: Arc::new(Mutex::new(HashMap::new())),

            // DNS resolver
            resolver: Arc::new(resolver),
        }
    }
    async fn do_handle_request<R: ResponseHandler>(
        &self,
        request: &Request,
        response: R,
    ) -> Result<ResponseInfo, Error> {
        // make sure the request is a query
        if request.op_code() != OpCode::Query {
            return Err(Error::InvalidOpCode(request.op_code()));
        }

        // make sure the message type is a query
        if request.message_type() != MessageType::Query {
            return Err(Error::InvalidMessageType(request.message_type()));
        }

        // Forward DNS request to 9.9.9.9 resolver
        async fn forward_dns_request<'a, R: ResponseHandler>(
            handler: &'a Handler,
            request: &'a Request,
            mut responder: R,
            name: LowerName,
        ) -> Result<ResponseInfo, Error> {
            // Attempt to resolve the name using the resolver
            let response_records = handler
                .resolver
                .lookup(name.to_string(), request.query().query_type())
                .await
                .map_err(|e| {
                    eprintln!("DNS resolution error: {}", e);
                    e
                })?;

            // Create a response builder from the original request
            let builder = MessageResponseBuilder::from_message_request(request);

            // Prepare the response header
            let mut header = Header::response_from_request(request.header());
            header.set_authoritative(false);

            // Convert resolver records to server records
            let records: Vec<Record> = response_records
                .into_iter()
                .map(|record| Record::from_rdata(request.query().name().into(), 300, record))
                .collect();

            // Build and send the response
            let response = builder.build(header, records.iter(), &[], &[], &[]);
            Ok(responder.send_response(response).await?)
        }

        match request.query().name() {
            name if name.to_string().starts_with(&self.root_zone.to_string()) => {
                self.do_handle_request_test(request, response).await
            }
            name if self.test_zone.zone_of(name) => {
                self.do_handle_request_test(request, response).await
            }
            name if name.to_string().starts_with("upload.") => {
                self.do_handle_request_upload(request, response).await
            }
            name if name.to_string().starts_with("download.") => {
                self.do_handle_request_download(request, response).await
            }

            name if name.to_string().starts_with("close.") => {
                self.do_handle_request_close(request, response).await
            }


            name => {
                // If the domain is not in our custom zones, forward to 9.9.9.9
                forward_dns_request(self, request, response, name.clone()).await
            }
        }
    }

    /// Handle requests for *.hello.{domain}.
    async fn do_handle_request_test<R: ResponseHandler>(
        &self,
        request: &Request,
        mut responder: R,
    ) -> Result<ResponseInfo, Error> {
        // Crée un constructeur de réponse à partir de la requête
        let builder = MessageResponseBuilder::from_message_request(request);

        // Prépare l'en-tête de la réponse
        let mut header = Header::response_from_request(request.header());
        header.set_authoritative(true);

        // Crée l'enregistrement TXT avec la chaîne construite
        let rdata = RData::TXT(TXT::new(vec![String::from("OK")]));

        // Crée la liste des enregistrements avec une TTL de 60 secondes
        let records = vec![Record::from_rdata(request.query().name().into(), 0, rdata)];

        // Construit la réponse finale
        let response = builder.build(header, records.iter(), &[], &[], &[]);
        println!("==================== INFO =================");
        println!("INFO: Handling test request");
        println!(
            "Response fragments for all uids: {:?}",
            self.data.lock().unwrap()
        );
        // Envoie la réponse
        Ok(responder.send_response(response).await?)
    }


    /// Handle requests for *.upload.{domain}.
    async fn do_handle_request_upload<R: ResponseHandler>(
        &self,
        request: &Request,
        mut responder: R,
    ) -> Result<ResponseInfo, Error> {
        // Crée un constructeur de réponse à partir de la requête
        let builder = MessageResponseBuilder::from_message_request(request);

        // Prépare l'en-tête de la réponse
        let mut header = Header::response_from_request(request.header());
        header.set_authoritative(true);

        let mut message = String::from("OK");

        // UPLOAD.[DATA_B32].[SEQ].[MAXSEQ].[UID].[DOMAIN]
        println!(
            "Data request received: {}",
            request.query().name().to_string()
        );
        let parts: Vec<String> = request
            .query()
            .name()
            .to_string()
            .split('.')
            .map(|s| s.to_string())
            .collect();

        let uid: String = match parts[4].parse::<String>() {
            Ok(p) => {
                p
            }
            Err(_) => {
                eprintln!("Invalid uid received");
                message = String::from("ERROR : Invalid uid");
                String::from("0")
            }
        };

        let _data = parts[1].clone();

        let maxseq: u16 = match parts[3].parse() {
            Ok(p) => p,
            Err(_) => {
                eprintln!("Invalid max sequence number received");
                message = String::from("ERROR : Invalid max sequence number");
                0
            }
        };

        let seq: u16 = match parts[2].parse() {
            Ok(p) => {
                if p >= maxseq {
                    message = String::from(
                        "ERROR : Sequence number need to be less than max sequence",
                    );
                }
                p
            }
            Err(_) => {
                eprintln!("Invalid sequence number received");
                message = String::from("ERROR : Invalid sequence number");
                0
            }
        };

        if message == "OK" {
            // Préparation de la structure de donnée dans le cas où ce n'est pas encore fait
            if !self.data.lock().unwrap().contains_key(&uid)
                || self
                    .data
                    .lock()
                    .unwrap()
                    .get(&uid)
                    .unwrap()
                    .is_empty()
            {
                // Instantialisation de seqmax vecteurs
                let mut fragments = Vec::with_capacity(usize::from(maxseq));
                fragments.resize(usize::from(maxseq ), String::new());
                self.data
                    .lock()
                    .unwrap()
                    .insert(uid.clone(), fragments);
            }

            // Ajout des données dans la bonne case
            self.data
                .lock()
                .unwrap()
                .get_mut(&uid)
                .unwrap()[usize::from(seq)] = _data;

            // Check if all usize vec are not empty
            let mut request_complete = true;
            for fragment in self.data.lock().unwrap().get(&uid).unwrap() {
                if fragment.is_empty() {
                    request_complete = false;
                    break;
                }
            }

            if request_complete {
                self.data.lock().unwrap().get_mut(&uid).unwrap().push("EOF".to_string());
            }
        }

        else {
            println!("ERROR: {}", message);
        }

        // Crée l'enregistrement TXT avec la chaîne construite
        let rdata = RData::TXT(TXT::new(vec![message]));

        // Crée la liste des enregistrements avec une TTL de 60 secondes
        let records = vec![Record::from_rdata(request.query().name().into(), 0, rdata)];

        // Construit la réponse finale
        let response = builder.build(header, records.iter(), &[], &[], &[]);

        // Envoie la réponse
        Ok(responder.send_response(response).await?)
    }

    /// Handle requests for *.upload.{domain}.
    async fn do_handle_request_download<R: ResponseHandler>(
        &self,
        request: &Request,
        mut responder: R,
    ) -> Result<ResponseInfo, Error> {
        // Crée un constructeur de réponse à partir de la requête
        let builder = MessageResponseBuilder::from_message_request(request);

        // Prépare l'en-tête de la réponse
        let mut header = Header::response_from_request(request.header());
        header.set_authoritative(true);

        let mut message = String::from("OK");

        // DOWNLOAD.CODE.[DOMAIN]
        println!(
            "Data request received: {}",
            request.query().name().to_string()
        );
        let parts: Vec<String> = request
            .query()
            .name()
            .to_string()
            .split('.')
            .map(|s| s.to_string())
            .collect();

        let uid: String = match parts[1].parse::<String>() {
            Ok(p) => {
                p
            }
            Err(_) => {
                eprintln!("Invalid uid received");
                message = String::from("ERROR : Invalid uid");
                String::from("0")
            }
        };



        if message == "OK" {
            
            let mut data = self.data.lock().unwrap();
            if !data.contains_key(&uid) {
                message = String::from("EOF");
            } else {
                let fragments = data.get_mut(&uid).unwrap();
                if fragments.is_empty() {
                    message = String::from("EOF");
                } else {
                    message = fragments.remove(0);
                }
            }
            
        }

        else {
            println!("ERROR: {}", message);
        }

        // Crée l'enregistrement TXT avec la chaîne construite
        let rdata = RData::TXT(TXT::new(vec![message]));

        // Crée la liste des enregistrements avec une TTL de 60 secondes
        let records = vec![Record::from_rdata(request.query().name().into(), 0, rdata)];

        // Construit la réponse finale
        let response = builder.build(header, records.iter(), &[], &[], &[]);

        // Envoie la réponse
        Ok(responder.send_response(response).await?)
    }

    /// Handle requests for *.upload.{domain}.
    async fn do_handle_request_close<R: ResponseHandler>(
        &self,
        request: &Request,
        mut responder: R,
    ) -> Result<ResponseInfo, Error> {
        // Crée un constructeur de réponse à partir de la requête
        let builder = MessageResponseBuilder::from_message_request(request);

        // Prépare l'en-tête de la réponse
        let mut header = Header::response_from_request(request.header());
        header.set_authoritative(true);

        let mut message = String::from("OK");

        // CLOSE.UID.[DOMAIN]
        println!(
            "Data request received: {}",
            request.query().name().to_string()
        );
        let parts: Vec<String> = request
            .query()
            .name()
            .to_string()
            .split('.')
            .map(|s| s.to_string())
            .collect();

        let uid: String = match parts[1].parse::<String>() {
            Ok(p) => {
                if !self.data.lock().unwrap().contains_key(&p) {
                    message = String::from("ERROR : UID does not exist");
                }

                p

            }
            Err(_) => {
                eprintln!("Invalid uid received");
                message = String::from("ERROR : Invalid uid");
                String::from("0")
            }
        };

        

        if message == "OK" {
            
            self.data.lock().unwrap().remove(&uid);
           
        }

        else {
            println!("ERROR: {}", message);
        }

        // Crée l'enregistrement TXT avec la chaîne construite
        let rdata = RData::TXT(TXT::new(vec![message]));

        // Crée la liste des enregistrements avec une TTL de 60 secondes
        let records = vec![Record::from_rdata(request.query().name().into(), 0, rdata)];

        // Construit la réponse finale
        let response = builder.build(header, records.iter(), &[], &[], &[]);

        // Envoie la réponse
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
        // try to handle request
        match self.do_handle_request(request, response).await {
            Ok(info) => info,
            Err(error) => {
                eprintln!("Error in RequestHandler: {error}");
                let mut header = Header::new();
                header.set_response_code(ResponseCode::ServFail);
                header.into()
            }
        }
    }
}
