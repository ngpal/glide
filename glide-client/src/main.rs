use regex::Regex;
use std::fs::File;
use std::io::{self, BufRead, Read};
use std::io::{Error, Write};
use std::path::Path;
use std::{env, fs};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;
use utils::commands::Command;
use utils::data::{ServerResponse, CHUNK_SIZE};

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
        } else if let Command::Glide { path, to: _ } = &command {
            if !matches!(response, ServerResponse::GlideRequestSent) {
                println!("Glide request failed! {}", response.to_string());
                return Ok(());
            }

            // Send file over to the server
            let metadata = fs::metadata(&path);

            // Send metadata
            match metadata {
                Ok(ref data) => {
                    stream
                        .write_all(
                            format!(
                                "{}:{}",
                                Path::new(&path).file_name().unwrap().to_string_lossy(),
                                data.len()
                            )
                            .as_bytes(),
                        )
                        .await?;
                    println!("Metadata sent!");
                }
                Err(e) => {
                    println!("There has been an error in locating the file:\n{}", e);
                    continue;
                }
            }

            // Calculate the number of chunks
            let file_length = metadata.unwrap().len();
            let partial_chunk_size = file_length % CHUNK_SIZE as u64;
            let chunk_count = file_length / CHUNK_SIZE as u64 + (partial_chunk_size > 0) as u64;

            // Read and send chunks
            let mut file = File::open(&path)?;
            let mut buffer = vec![0; CHUNK_SIZE];
            for count in 0..chunk_count {
                let bytes_read = file.read(&mut buffer)?;
                if bytes_read == 0 {
                    break;
                }
                stream.write_all(&buffer[..bytes_read]).await?;
                println!(
                    "Sent chunk {}/{} ({}%)\r",
                    count + 1,
                    chunk_count,
                    ((count + 1) as f64 / chunk_count as f64 * 100.0) as u8
                );
            }

            println!("\nFile upload completed successfully!");
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

    ServerResponse::from(&String::from_utf8_lossy(&response))
}

fn validate_username(username: &str) -> bool {
    let re = Regex::new(r"^[a-zA-Z0-9](?:[a-zA-Z0-9\.]{0,8}[a-zA-Z0-9])?$").unwrap();
    re.is_match(username)
}
