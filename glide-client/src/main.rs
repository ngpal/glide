use regex::Regex;
use std::env;
use std::io::{self, BufRead};
use std::io::{Error, Write};
use std::path::Path;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;
use utils::commands::Command;
use utils::data::{ServerResponse, CHUNK_SIZE};
use utils::transfers;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Retrieve command-line arguments
    let args: Vec<String> = env::args().collect();

    // Check if the required arguments are provided
    if args.len() != 3 {
        eprintln!("Usage: {} <IP> <PORT>", args[0]);
        std::process::exit(1);
    }

    // Parse the IP and port from arguments
    let ip = &args[1];
    let port = &args[2];
    let address = format!("{}:{}", ip, port);

    // Connect to the server
    let mut stream = TcpStream::connect(&address).await?;
    println!("Connected to server at {}!", address);

    let _username = login(&mut stream).await?;

    // Command loop
    let stdin = io::stdin();
    let mut input = String::new();

    println!("Type 'help' to see available commands.");

    loop {
        // Get user input
        input.clear();
        print!("glide> ");
        io::stdout().flush()?;
        stdin.lock().read_line(&mut input)?;

        let input = input.trim();
        if input == "exit" {
            println!("Thank you for using Glide. Goodbye!");
            break;
        }

        // Parse the command
        let command = Command::parse(input);

        // Validate glide command
        if let Command::Glide { path, to: _ } = &command {
            // Check if file exists
            if Path::new(&path).try_exists().is_err() || !Path::new(&path).is_file() {
                println!("Path '{}' is invalid. File does not exist", path);
                continue;
            }
        }

        // Send command to the server
        stream.write_all(input.as_bytes()).await?;
        let response = get_server_response(&mut stream).await?;

        if matches!(response, ServerResponse::UnknownCommand) {
            println!("Invalid command '{}'. Use 'help' to see more", input);
            continue;
        }

        match command {
            Command::Glide { path, to: _ } => {
                transfers::send_file(&mut stream, &path).await?;
            }
            Command::Ok(_) => {
                if matches!(response, ServerResponse::OkSuccess) {
                    println!("Getting file...");
                } else {
                    println!("`ok` failed :(");
                }

                transfers::receive_file(&mut stream, ".").await?;
            }
            Command::List => {
                let ServerResponse::ConnectedUsers(users) = response else {
                    println!("Command failed\n{}", response.to_string());
                    return Ok(());
                };

                println!("Connected users:");
                for user in users.iter() {
                    println!(" @{}", user);
                }
            }
            Command::Requests => {
                let ServerResponse::IncomingRequests(reqs) = response else {
                    println!("Command failed\n{}", response.to_string());
                    return Ok(());
                };

                println!("Incoming requests:");
                for req in reqs.iter() {
                    println!(" From: {}, File: {}", req.from_username, req.filename);
                }
            }
            _ => {}
        }
    }

    Ok(())
}

async fn login(stream: &mut TcpStream) -> Result<String, Box<dyn std::error::Error>> {
    let mut username = String::new();

    loop {
        username.clear();
        print!("Enter your username: ");
        io::stdout().flush()?;
        io::stdin().read_line(&mut username)?;

        let username = username.trim();

        if !validate_username(username) {
            println!(
                "Invalid username!
Usernames must follow these rules:
    • Only alphanumeric characters and periods (.) are allowed.
    • Must be 1 to 10 characters long.
    • Cannot start or end with a period (.).
    • Cannot contain consecutive periods (..).

Please try again with a valid username."
            );
            continue;
        }

        // Send the username to the server
        stream.write_all(username.as_bytes()).await?;

        // Wait for the server's response
        let response = get_server_response(stream).await?;
        if matches!(response, ServerResponse::UsernameOk) {
            println!("You are now connected as @{}", username);
            break;
        } else {
            println!("Server rejected username: {}", response.to_string());
        }
    }

    Ok(username)
}

async fn get_server_response(stream: &mut TcpStream) -> Result<ServerResponse, Error> {
    let mut response = vec![0; CHUNK_SIZE];
    let bytes_read = stream.read(&mut response).await?;
    if bytes_read == 0 {
        println!("Server disconnected unexpectedly.");
        return Err(Error::new(
            io::ErrorKind::Other,
            "Connection closed by server",
        ));
    }

    ServerResponse::from(&String::from_utf8_lossy(&response)[..bytes_read])
}

fn validate_username(username: &str) -> bool {
    let re = Regex::new(r"^[a-zA-Z0-9](?:[a-zA-Z0-9\.]{0,8}[a-zA-Z0-9])?$").unwrap();
    re.is_match(username)
}
