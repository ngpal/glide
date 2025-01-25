# Glide v1.0.1
Glide is an app that runs on your LAN network, letting you share files with anyone connected to the same network as you are, without any internet at all! It's super easy to set up and use.
## Setup
You need to have `cargo` installed to compile and run the server and the client as of now, unfortunately. Let's start by cloning this repository locally 
```bash
git clone https://www.github.com/ngpal/glide.git
cd glide
```
### Setting up the server
To set up the server all you have to do is
```bash
cargo run --release
```
Or if you want information about who is connecting from where run with
```bash
RUST_LOG=info cargo run --release
```
### Connecting your client
To connect your client to the server, first clone the cli client repository and run the following command with `<IP>` and `<PORT>` as the same ones given when running the server
```bash
git clone https://www.github.com/ngpal/glide-cli.git
cd glide-cli
cargo run --release -- <IP> <PORT>
```
## Commands
Glide uses simple and easy to remember commands and syntax to interract with the server and other users connected to the server. You can start by typing in `list` to see the list of all the other
users connected to the server. 
### `list`
The `list` command, as described above, shows you the usernames of all the users that are currently connected to the server.
### `glide path/to/file.ext @username`
The `glide` command is used to initialize file transfer between users, by specifying the path to the file you want to share, and the username of the recepient of the file. This will send the file
to the server, and a request to the recipient, which they can later accept to go on with the file transfer.
### `reqs`
You can use the `reqs` command to show a list of all the incoming requests, filename, and the user it is coming from.
### `ok @username`
The `ok` command is used to accept a glide request from `@username` and download the file from the server, and complete the file transfer process.
### `no @username`
The `no` command is used to reject a glide request from `@username`, deleting the file from the server.
### `exit`
Use the `exit` command to gracefully exit the client program, and disconnect from the server.
