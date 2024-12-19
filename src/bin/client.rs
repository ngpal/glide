use std::io;
use tokio::io::AsyncWriteExt;
use tokio::net::TcpStream;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut stream = TcpStream::connect("127.0.0.1:8080").await?;
    println!("Connected to server");

    loop {
        let mut message = String::new();
        println!("Enter a message to send (or type 'exit' to quit):");

        io::stdin().read_line(&mut message)?;

        let message = message.trim();

        if message.to_lowercase() == "exit" {
            println!("Exiting...");
            break;
        }

        stream.write_all(message.as_bytes()).await?;
        println!("Message sent!");
    }

    Ok(())
}
