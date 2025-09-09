# kpasswd - Kanidm Password Changing Tool

A simple utility for changing Kanidm user Unix passwords with SGID privileges.

## Features

- Runs with SGID permissions to allow changing passwords via privileged admin account
- Identifies the calling user by UID

## Installation

1. Clone this repository
2. Run the installation script:
   ```
   cargo build
   ```
3. Edit the configuration file with your Kanidm server details:
   ```
   sudo nano /etc/kanidm/kpasswd
   ```

## Configuration

The `/etc/kanidm/kpasswd.toml` file should contain:

```toml
server_url = "https://your-kanidm-server.com"
admin_username = "admin_account"
admin_password = "admin_password"
```

Make sure the file has secure permissions:
```
sudo chmod 640 /etc/kanidm/kpasswd.toml
sudo chown root:shadow /etc/kanidm/kpasswd /usr/local/bin/kanidm
sudo chmod g+s /usr/local/bin/kanidm
```

## Usage

Simply run:

```
kpasswd
```

The tool will:
1. Identify your username from your UID
2. Prompt for a new password
3. Confirm the password
4. Change your password on the Kanidm server

## Security Notes

- The binary should have the SGID bit set to allow password changing with elevated privileges
- The configuration file should be readable only by root and the privileged group (shadow)
