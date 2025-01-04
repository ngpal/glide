#[derive(Clone, Debug)]
pub struct Request {
    pub from_username: String,
    pub filename: String,
}

#[derive(Debug)]
pub struct UserData {
    pub socket: String,
    pub incoming_requests: Vec<Request>,
}

#[derive(Debug)]
pub enum ServerSignal {
    UsernameInvalid,
    UsernameTaken,
    UsernameOk,
    UnknownCommand,
    ConnectedUsers(Vec<String>),
    IncomingRequests(Vec<Request>),
    GlideRequestSent,
}

impl ServerSignal {
    pub fn from(string: &str) -> Result<ServerSignal, String> {
        let signal = match string {
            "INVALID_USERNAME" => Self::UsernameInvalid,
            "USERNAME_TAKEN" => Self::UsernameTaken,
            "USERNAME_OK" => Self::UsernameOk,
            "UNKNOWN_COMMAND" => Self::UnknownCommand,
            "GLIDE_REQ_OK" => Self::GlideRequestSent,
            // Eg: CONNECTED_USERS user1 user2 user3
            x if x.starts_with("CONNECTED_USERS ") => Self::ConnectedUsers(
                x["CONNECTED_USERS ".len()..]
                    .split_whitespace()
                    .map(String::from)
                    .collect(),
            ),

            // Eg: INCOMING_REQUESTS user1:filename user2:filename
            x if x.starts_with("INCOMING_REQUESTS ") => Self::IncomingRequests(
                x["INCOMING_REQUESTS ".len()..]
                    .split_whitespace()
                    .map(|entry| {
                        let (from_username, filename) = entry.split_once(":").unwrap();
                        Request {
                            from_username: from_username.to_string(),
                            filename: filename.to_string(),
                        }
                    })
                    .collect(),
            ),

            _ => return Err(format!("Unable to parse {}", string)),
        };

        Ok(signal)
    }

    pub fn to_string(self) -> String {
        match self {
            Self::UsernameInvalid => "INVALID_USERNAME".to_string(),
            Self::UsernameTaken => "USERNAME_TAKEN".to_string(),
            Self::UsernameOk => "USERNAME_OK".to_string(),
            Self::UnknownCommand => "UNKNOWN_COMMAND".to_string(),
            Self::GlideRequestSent => "GLIDE_REQ_OK".to_string(),

            Self::ConnectedUsers(users) => {
                format!("CONNECTED_USERS {}", users.join(" "))
            }

            Self::IncomingRequests(requests) => {
                let formatted_requests = requests
                    .into_iter()
                    .map(|req| format!("{}:{}", req.from_username, req.filename))
                    .collect::<Vec<_>>()
                    .join(" ");
                format!("INCOMING_REQUESTS {}", formatted_requests)
            }
        }
    }
}
