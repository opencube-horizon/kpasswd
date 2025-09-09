use anyhow::{anyhow, Result};
use kanidm_client::{ClientError, KanidmClientBuilder, StatusCode};
use rpassword::read_password;
use serde::Deserialize;
use std::fs::File;
use std::io::{self, Read, Write};
use std::path::Path;
use tokio;
use users::get_current_username;

#[derive(Deserialize)]
struct KpasswdConfig {
    server_url: String,
    admin_username: String,
    admin_password: String,
}

fn read_config_file(path: &Path) -> Result<KpasswdConfig> {
    let mut file = File::open(path)?;
    let mut contents = String::new();
    file.read_to_string(&mut contents)?;

    let config: KpasswdConfig = toml::from_str(&contents)?;
    Ok(config)
}

async fn change_password(username: &str, new_password: &str, config: &KpasswdConfig) -> Result<()> {
    // Create client to connect to Kanidm server
    let client = match KanidmClientBuilder::new()
        .address(config.server_url.clone())
        .build()
    {
        Ok(val) => val,
        Err(err) => return Err(anyhow!("Creating Kanidm client failed: {:?}", err)),
    };

    // Authenticate as admin
    let r = client
        .auth_simple_password(&config.admin_username, &config.admin_password)
        .await;

    if r.is_err() {
        match r {
            Err(ClientError::Transport(value)) => {
                return Err(anyhow!("Failed to connect to kanidm server: {}", value.to_string()));
            }
            _ => {
                return Err(anyhow!("Error during authentication phase: {:?}", r));
            }
        }
    }

    if let Err(e) = client
        .idm_person_account_unix_cred_put(username, new_password)
        .await
    {
        match e {
            ClientError::Http(status, error, opid) => {
                let error_msg = match &error {
                    Some(msg) => format!(" {msg:?}"),
                    None => "".to_string(),
                };
                eprintln!("OperationId: {:?}", opid);
                if status == StatusCode::INTERNAL_SERVER_ERROR {
                    return Err(anyhow!("Internal Server Error in response: {}", error_msg));
                } else if status == StatusCode::NOT_FOUND {
                    return Err(anyhow!("User not found."));
                } else {
                    return Err(anyhow!("HTTP Error: {}{}", status, error_msg));
                }
            }
            ClientError::Transport(e) => {
                return Err(anyhow!("HTTP-Transport Related Error: {:?}", e));
            }
            ClientError::UntrustedCertificate(e) => {
                return Err(anyhow!("Untrusted Certificate Error: {:?}", e));
            }
            _ => {
                return Err(anyhow!("{e:?}"));
            }
        };
    }

    Ok(())
}

#[tokio::main]
async fn main() -> Result<()> {
    let username = match get_current_username() {
        Some(v) => v,
        None => {
            return Err(anyhow!("No username?!"));
        }
    };

    println!("Changing password for user: {}", username.to_string_lossy());

    // Read admin credentials from config file
    let config = read_config_file(Path::new("/etc/kanidm/kpasswd"))?;

    // Ask for new password
    print!("New Password: ");
    io::stdout().flush()?;
    let new_password = read_password()?;

    print!("Confirm Password: ");
    io::stdout().flush()?;
    let confirm_password = read_password()?;

    // Verify passwords match
    if new_password != confirm_password {
        return Err(anyhow!("Passwords do not match"));
    }

    // Change the password
    match change_password(&username.to_string_lossy(), &new_password, &config).await {
        Ok(_) => {
            println!("Password changed successfully");
            Ok(())
        }
        Err(e) => {
            println!("Failed to change password: {}", e);
            Err(anyhow!("Password change failed: {}", e))
        }
    }
}
