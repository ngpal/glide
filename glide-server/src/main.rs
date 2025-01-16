#![allow(unused)]

use get_if_addrs::{get_if_addrs, IfAddr};
use regex::Regex;
use std::collections::HashMap;
use std::fs;
use std::os::unix::fs::MetadataExt;
use std::path::Path;
use std::sync::Arc;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::Mutex;
use utils::data::ServerResponse;

use utils::{
    commands::Command,
    data::{Request, UserData},
};

const CHUNK_SIZE: usize = 1024;

// Shared state to hold usernames and requests of connected clients
type SharedState = Arc<Mutex<HashMap<String, UserData>>>;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut network_ip = "127.0.0.1".to_string(); // Default to localhost

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
    println!("  Network clients: http://{}:8000", network_ip);

    let listener = TcpListener::bind("0.0.0.0:8000").await?;
    println!("Server is running on 0.0.0.0:8000 (listening on all interfaces)");
    let state: SharedState = Arc::new(Mutex::new(HashMap::new()));

    loop {
        let (mut socket, addr) = listener.accept().await?;
        let state = Arc::clone(&state);

        println!("New connection from: {}", addr);

        tokio::spawn(async move {
            if let Err(e) = handle_client(&mut socket, state).await {
                eprintln!("Error handling client {}: {}", addr, e);
            }
        });
    }
}

async fn handle_client(
    socket: &mut TcpStream,
    state: SharedState,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut buffer = vec![0; CHUNK_SIZE];
    let mut username = String::new();

    // Loop until a valid username is provided
    loop {
        let bytes_read = socket.read(&mut buffer).await?;
        if bytes_read == 0 {
            return Ok(()); // Client disconnected
        }

        username.clear();
        username.push_str(String::from_utf8_lossy(&buffer[..bytes_read]).trim());

        // Check if the username is valid and available
        let response = {
            let clients = state.lock().await;
            if !validate_username(&username) {
                ServerResponse::UsernameInvalid
            } else if clients.contains_key(&username) {
                ServerResponse::UsernameTaken
            } else {
                drop(clients);
                add_client(&username, socket, &state).await?;
                ServerResponse::UsernameOk
            }
        };

        // Send the response to the client
        socket.write_all(response.to_string().as_bytes()).await?;

        if matches!(response, ServerResponse::UsernameOk) {
            println!("Client @{} connected", username);
            break;
        }
    }

    // Start command handling loop (e.g., list, help, etc.)
    loop {
        let bytes_read = socket.read(&mut buffer).await?;
        if bytes_read == 0 {
            remove_client(&username, &state).await;
            break;
        }

        let command = String::from_utf8_lossy(&buffer[..bytes_read])
            .trim()
            .to_string();

        if let Err(e) = Command::handle(&command, &username, socket, &state).await {
            println!("Error handling command for @{}: {}", username, e);
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
            if req.from_username == username {
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
    fs::remove_dir_all(username);

    println!("Client @{} disconnected", username);
}

fn validate_username(username: &str) -> bool {
    let re = Regex::new(r"^[a-zA-Z0-9](?:[a-zA-Z0-9\.]{0,8}[a-zA-Z0-9])?$").unwrap();
    re.is_match(username)
}
