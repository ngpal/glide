use regex::Regex;
use std::env;
use std::io::Write;
use std::io::{self, BufRead};
use std::path::Path;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;
use utils::commands::Command;

const CHUNK_SIZE: usize = 1024;

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

        // Await server response
        let mut response = vec![0; CHUNK_SIZE];
        let bytes_read = stream.read(&mut response).await?;
        if bytes_read == 0 {
            println!("Server disconnected.");
            break;
        }

        // Print server response
        let response_str = String::from_utf8_lossy(&response[..bytes_read]);
        println!("{}", response_str);

        // Block client until OK signal
        if let Command::Glide { path, to } = &command {
            if !response_str.starts_with("Successfully") {
                return Ok(());
            }

            println!("Waiting for @{} to respond...", to);

            // Await server response
            let mut response = vec![0; CHUNK_SIZE];
            let bytes_read = stream.read(&mut response).await?;
            if bytes_read == 0 {
                println!("Server disconnected.");
                break;
            }
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
        let mut response = vec![0; CHUNK_SIZE];
        let bytes_read = stream.read(&mut response).await?;
        if bytes_read == 0 {
            println!("Server disconnected unexpectedly.");
            return Err("Connection closed by the server".into());
        }

        let response_str = String::from_utf8_lossy(&response[..bytes_read])
            .trim()
            .to_string();

        if response_str == "OK" {
            println!("You are now connected as @{}", username);
            break;
        } else {
            println!("Server rejected username: {}", response_str);
        }
    }

    Ok(username)
}

fn validate_username(username: &str) -> bool {
    let re = Regex::new(r"^[a-zA-Z0-9](?:[a-zA-Z0-9\.]{0,8}[a-zA-Z0-9])?$").unwrap();
    re.is_match(username)
}
