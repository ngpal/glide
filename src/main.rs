use get_if_addrs::{get_if_addrs, IfAddr};
use log::{error, info, warn};
use regex::Regex;
use std::collections::HashMap;
use std::fs::{self};
use std::sync::Arc;
use tokio::io::AsyncWriteExt;
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::Mutex;
use utils::{commands::Command, data::UserData, protocol::Transmission};

const CHUNK_SIZE: usize = 1024;

// Shared state to hold usernames and requests of connected clients
type SharedState = Arc<Mutex<HashMap<String, UserData>>>;

struct Cleanup;
impl Drop for Cleanup {
    fn drop(&mut self) {
        info!("Clearing clients folder");
        fs::remove_dir_all("./clients");
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut network_ip = "127.0.0.1".to_string(); // Default to localhost
    env_logger::init();

    for iface in get_if_addrs()? {
        if let IfAddr::V4(v4_addr) = iface.addr {
            // Use the first non-loopback IPv4 address as the network IP
            if !v4_addr.ip.is_loopback() && network_ip == "127.0.0.1" {
                network_ip = v4_addr.ip.to_string();
            }
        }
    }

    // Display the message with the dynamically detected IP
    println!("To connect to this server, use the following address:");
    println!("  Local clients: http://127.0.0.1:8000");
    println!("  Network clients: http://{}:8000\n", network_ip);

    let listener = TcpListener::bind("0.0.0.0:8000").await?;
    info!("Server is running on 0.0.0.0:8000 (listening on all interfaces)");
    let state: SharedState = Arc::new(Mutex::new(HashMap::new()));
    fs::create_dir("./clients")?;

    let cleaner = Cleanup;

    loop {
        let (mut socket, addr) = listener.accept().await?;
        let state = Arc::clone(&state);

        info!("New connection from: {}", addr);

        tokio::spawn(async move {
            if let Err(e) = handle_client(&mut socket, state).await {
                error!("Error handling client {}: {}", addr, e);
            }
        });
    }
}

async fn handle_client(
    stream: &mut TcpStream,
    state: SharedState,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut username = String::new();

    // Loop until a valid username is provided
    loop {
        let input = Transmission::from_stream(stream).await?;
        username = if let Transmission::Username(uname) = input {
            uname
        } else {
            continue;
        };
        // Check if the username is valid and available
        let response = {
            let clients = state.lock().await;
            if !validate_username(&username) {
                Transmission::UsernameInvalid
            } else if clients.contains_key(&username) {
                Transmission::UsernameTaken
            } else {
                drop(clients);
                add_client(&username, stream, &state).await?;
                Transmission::UsernameOk
            }
        };

        // Send the response to the client
        stream.write_all(response.to_bytes().as_slice()).await?;

        if matches!(response, Transmission::UsernameOk) {
            info!("Client @{} connected", username);
            break;
        }
    }

    // Start command handling loop (e.g., list, help, etc.)
    loop {
        let command;

        match Transmission::from_stream(stream).await? {
            Transmission::Command(cmd) => command = cmd,
            Transmission::ClientDisconnected => {
                remove_client(&username, &state).await;
                break;
            }
            something_else => {
                warn!(
                    "Didn't recieve command when I should have\n{:#?}",
                    something_else
                );
                continue;
            }
        }

        if let Err(e) = Command::handle(command, &username, stream, &state).await {
            error!("Error handling command for @{}: {}", username, e);
            break;
        }
    }

    Ok(())
}

async fn add_client(
    username: &str,
    socket: &mut TcpStream,
    state: &SharedState,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut clients = state.lock().await;
    clients.insert(
        username.to_string(),
        UserData {
            socket: socket.peer_addr()?.to_string(),
            incoming_requests: vec![],
        },
    );
    Ok(())
}

async fn remove_client(username: &str, state: &SharedState) {
    let mut clients = state.lock().await;

    // Remove the client
    clients.remove(username);

    // Collect requests to be removed
    let mut to_remove = Vec::new();
    for (user, client) in clients.iter() {
        for (i, req) in client.incoming_requests.iter().enumerate() {
            if req.sender == username {
                to_remove.push((user.clone(), i));
            }
        }
    }

    // Remove the collected requests
    for (user, index) in to_remove {
        if let Some(client) = clients.get_mut(&user) {
            client.incoming_requests.remove(index);
        }
    }

    // Remove folder under user
    fs::remove_dir_all(username).unwrap();

    info!("Client @{} disconnected", username);
}

fn validate_username(username: &str) -> bool {
    let re = Regex::new(r"^[a-zA-Z0-9](?:[a-zA-Z0-9\.]{0,8}[a-zA-Z0-9])?$").unwrap();
    re.is_match(username)
}
