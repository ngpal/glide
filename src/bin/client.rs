use std::io::Write;
use std::{fs, io};
use tokio::io::AsyncWriteExt;
use tokio::net::TcpStream;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut stream = TcpStream::connect("127.0.0.1:8080").await?;
    println!("Connected to server");

    loop {
        let mut file_name = String::new();

        print!("Enter the file name to send (or type 'exit' to quit): ");
        io::stdout().flush().unwrap();

        io::stdin().read_line(&mut file_name)?;

        let file_name = file_name.trim();
        if file_name.to_lowercase() == "exit" {
            println!("Exiting...");
            break;
        }

        match fs::metadata(&file_name) {
            Ok(metadata) => {
                stream
                    .write_all(format!("{}:{}", &file_name, metadata.len()).as_bytes())
                    .await?;
                println!("Message sent!");
            }
            Err(e) => {
                println!("There has been an error in locating the file:\n{}", e);
            }
        }
    }

    Ok(())
}
