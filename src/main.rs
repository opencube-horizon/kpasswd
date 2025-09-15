use anyhow::{anyhow, Result};
use clap::Parser;
use kanidm_client::{ClientError, KanidmClientBuilder, StatusCode};
use kanidm_proto::attribute::Attribute;
use kanidm_proto::scim_v1::{client::ScimSshPublicKeys, ScimEntryGetQuery};
use rpassword::read_password;
use serde::Deserialize;
use std::fs::File;
use std::io::{self, Read, Write};
use std::path::Path;
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

async fn create_kanidm_client(config: &KpasswdConfig) -> Result<kanidm_client::KanidmClient> {
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
                return Err(anyhow!(
                    "Failed to connect to kanidm server: {}",
                    value.to_string()
                ));
            }
            _ => {
                return Err(anyhow!("Error during authentication phase: {:?}", r));
            }
        }
    }

    Ok(client)
}

fn handle_client_error(e: ClientError) -> Result<()> {
    match e {
        ClientError::Http(status, error, opid) => {
            let error_msg = match &error {
                Some(msg) => format!(" {msg:?}"),
                None => "".to_string(),
            };
            eprintln!("OperationId: {:?}", opid);
            if status == StatusCode::INTERNAL_SERVER_ERROR {
                Err(anyhow!("Internal Server Error in response: {}", error_msg))
            } else if status == StatusCode::NOT_FOUND {
                Err(anyhow!("User not found."))
            } else {
                Err(anyhow!("HTTP Error: {}{}", status, error_msg))
            }
        }
        ClientError::Transport(e) => Err(anyhow!("HTTP-Transport Related Error: {:?}", e)),
        ClientError::UntrustedCertificate(e) => {
            Err(anyhow!("Untrusted Certificate Error: {:?}", e))
        }
        _ => Err(anyhow!("{e:?}")),
    }
}

async fn change_password(username: &str, new_password: &str, config: &KpasswdConfig) -> Result<()> {
    let client = create_kanidm_client(config).await?;

    if let Err(e) = client
        .idm_person_account_unix_cred_put(username, new_password)
        .await
    {
        handle_client_error(e)?;
    }

    Ok(())
}

async fn add_ssh_key(username: &str, ssh_key: &str, config: &KpasswdConfig) -> Result<()> {
    let client = create_kanidm_client(config).await?;

    // Extract the key comment/description to use as tag if available
    // Split at most into 3 parts (type, data, comment)
    let parts: Vec<&str> = ssh_key.trim().splitn(3, ' ').collect();

    // SSH keys typically have format: type base64-data [comment]
    // Use the comment as the tag if available, or generate one
    let tag = if parts.len() >= 3 {
        parts[2].to_string()
    } else {
        // Generate a tag from the key content
        format!("key_{}", ssh_key.chars().take(8).collect::<String>())
    };

    if let Err(e) = client
        .idm_person_account_post_ssh_pubkey(username, &tag, ssh_key)
        .await
    {
        return handle_client_error(e);
    }

    Ok(())
}

async fn list_ssh_keys(username: &str, config: &KpasswdConfig) -> Result<()> {
    let client = create_kanidm_client(config).await?;

    // the following was lifted directly from Kanidm
    let mut entry = match client
        .scim_v1_person_get(
            username,
            Some(ScimEntryGetQuery {
                attributes: Some(vec![Attribute::SshPublicKey]),
                ..Default::default()
            }),
        )
        .await
    {
        Ok(entry) => entry,
        Err(e) => return handle_client_error(e),
    };

    let Some(pkeys) = entry.attrs.remove(&Attribute::SshPublicKey) else {
        println!("No ssh public keys");
        return Ok(());
    };

    let Ok(keys) = serde_json::from_value::<ScimSshPublicKeys>(pkeys) else {
        eprintln!("Invalid ssh public key format");
        return Err(anyhow!("Invalid ssh public key format found"));
    };

    for key in keys {
        println!("Kanidm Key ID '{}': {}", key.label, key.value);
    }
    Ok(())
}

async fn delete_ssh_key(username: &str, tag: &str, config: &KpasswdConfig) -> Result<()> {
    let client = create_kanidm_client(config).await?;

    if let Err(e) = client
        .idm_person_account_delete_ssh_pubkey(username, tag)
        .await
    {
        return handle_client_error(e);
    }

    Ok(())
}

#[derive(Parser)]
#[command(about = "Lets users change their POSIX/sudo password and manage keys in a primarily password-less Kanidm managed environment", long_about = None)]
struct Cli {
    /// Add an SSH public key (as it appears in ~/.ssh/authorized_keys)
    #[arg(short = 'a', long, value_name = "SSH_KEY")]
    add_ssh_key: Option<String>,

    /// List all SSH keys
    #[arg(short = 'l', long)]
    list_ssh_keys: bool,

    /// Delete an SSH key with the given tag/description
    #[arg(short = 'd', long, value_name = "ID")]
    delete_ssh_key: Option<String>,
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();

    let username = match get_current_username() {
        Some(v) => v,
        None => {
            return Err(anyhow!("No username?!"));
        }
    };

    // Read admin credentials from config file
    let config = read_config_file(Path::new("/etc/kanidm/kpasswd"))?;

    // Handle SSH key management options
    if let Some(ssh_key) = cli.add_ssh_key {
        match add_ssh_key(&username.to_string_lossy(), &ssh_key, &config).await {
            Ok(_) => {
                println!("SSH key added successfully");
            }
            Err(e) => {
                eprintln!("Failed to add SSH key: {}", e);
                return Err(anyhow!("SSH key addition failed: {}", e));
            }
        }
    } else if cli.list_ssh_keys {
        // List SSH keys
        match list_ssh_keys(&username.to_string_lossy(), &config).await {
            Ok(_) => {}
            Err(e) => {
                eprintln!("Failed to list SSH keys: {}", e);
                return Err(anyhow!("SSH key listing failed: {}", e));
            }
        }
    } else if let Some(tag) = cli.delete_ssh_key {
        // Delete SSH key by tag
        match delete_ssh_key(&username.to_string_lossy(), &tag, &config).await {
            Ok(_) => {
                println!("SSH key deleted successfully");
            }
            Err(e) => {
                eprintln!("Failed to delete SSH key: {}", e);
                return Err(anyhow!("SSH key deletion failed: {}", e));
            }
        }
    } else {
        // Default behavior: Change Unix password
        println!("Changing password for user: {}", username.to_string_lossy());

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
            }
            Err(e) => {
                eprintln!("Failed to change password: {}", e);
                return Err(anyhow!("Password change failed: {}", e));
            }
        }
    }

    Ok(())
}
