use regex::Regex;
use std::env;
use std::io::Write;
use std::io::{self, BufRead};
use std::path::Path;
use tokio::io::AsyncWriteExt;
use tokio::net::TcpStream;
use utils::commands::Command;
use utils::protocol::Transmission;
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
            stream
                .write_all(Transmission::ClientDisconnected.to_bytes().as_slice())
                .await?;
            break;
        }

        // Parse the command
        let command = Command::parse(input);

        if !validate_command(&command.to_string()) {
            println!("Invalid command '{}'. Use 'help' to see more", input);
            continue;
        }

        // Validate glide command
        if let Command::Glide { path, to: _ } = &command {
            // Check if file exists
            if Path::new(&path).try_exists().is_err() || !Path::new(&path).is_file() {
                println!("Path '{}' is invalid. File does not exist", path);
                continue;
            }
        }

        // Send command to the server
        stream
            .write_all(Transmission::Command(command.clone()).to_bytes().as_slice())
            .await?;
        let response = Transmission::from_stream(&mut stream).await?;

        match command {
            Command::Glide { path, to: _ } => {
                if matches!(response, Transmission::GlideRequestSent) {
                    transfers::send_file(&mut stream, &path).await?;
                } else if matches!(response, Transmission::UsernameInvalid) {
                    println!("Unable to send glide request, username invalid");
                } else {
                    println!("Unable to send glide request\n{:#?}", response);
                }
            }
            Command::Ok(_) => {
                if matches!(response, Transmission::OkSuccess) {
                    transfers::receive_file(&mut stream, ".").await?;
                } else {
                    println!("`ok` command failed! Invalid request")
                }
            }
            Command::List => {
                let Transmission::ConnectedUsers(users) = response else {
                    println!("Command failed\n{:#?}", response);
                    return Ok(());
                };

                println!("Connected users:");
                for user in users.iter() {
                    println!(" @{}", user);
                }
            }
            Command::Requests => {
                let Transmission::IncomingRequests(reqs) = response else {
                    println!("Command failed\n{:#?}", response);
                    return Ok(());
                };

                println!("Incoming requests:");
                for req in reqs.iter() {
                    println!(" From: {}, File: {}", req.sender, req.filename);
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
        stream
            .write_all(
                Transmission::Username(username.to_string())
                    .to_bytes()
                    .as_slice(),
            )
            .await?;

        // Wait for the server's response
        let response = Transmission::from_stream(stream).await?;
        if matches!(response, Transmission::UsernameOk) {
            println!("You are now connected as @{}", username);
            break;
        } else {
            println!(
                "Server rejected username: {}",
                match response {
                    Transmission::UsernameTaken => "Username is taken",
                    Transmission::UsernameInvalid => "Username is invalid",
                    _ => unreachable!(),
                }
            );
        }
    }

    Ok(username)
}

fn validate_username(username: &str) -> bool {
    let re = Regex::new(r"^[a-zA-Z0-9](?:[a-zA-Z0-9\.]{0,8}[a-zA-Z0-9])?$").unwrap();
    re.is_match(username)
}

pub fn validate_command(input: &str) -> bool {
    let list_re = Regex::new(r"^list$").unwrap();
    let reqs_re = Regex::new(r"^reqs$").unwrap();
    let glide_re = Regex::new(r"^glide\s+.+\s+@\S+$").unwrap();
    let ok_re = Regex::new(r"^ok\s+@\S+$").unwrap();
    let no_re = Regex::new(r"^no\s+@\S+$").unwrap();

    list_re.is_match(input)
        || reqs_re.is_match(input)
        || glide_re.is_match(input)
        || ok_re.is_match(input)
        || no_re.is_match(input)
}
