# kpasswd - Kanidm Password Changing Tool

Small utility to set the Unix (sudo) password for Kanidm users.
Optionally "authenticated" by UID only, for an otherwise password-less login infrastructure.

## Features

- Runs with SGID permissions to allow changing passwords via privileged admin account
- Identifies the calling user by UID

## Installation

1. Clone this repository
2. Build it:
   ```console
   ~> cargo build
   ```
3. Create the configuration file with your Kanidm server details and a privileged user:
   ```console
   ~> sudo nano /etc/kanidm/kpasswd
   ```

## Configuration

The `/etc/kanidm/kpasswd.toml` file should contain:

```toml
server_url = "https://your-kanidm-server.com"
admin_username = "admin_account"
admin_password = "admin_password"

# Set to true to allow setting a password without providing the current one.
# Only enable this during initial rollout when users don't have passwords yet.
# allow_set_without_current = false
```

Make sure the file has secure permissions:
```console
~> sudo chmod 640 /etc/kanidm/kpasswd.toml
~> sudo chown root:shadow /etc/kanidm/kpasswd /usr/local/bin/kanidm
~> sudo chmod g+s /usr/local/bin/kanidm
```

## Usage

Simply run:

```console
~> kpasswd
```

The tool will:
1. Identify your username from your UID
2. Prompt for a new password
3. Confirm the password
4. Change your password on the Kanidm server

The tool can also manage the users SSH keys:

```console
~> kpasswd --help
Usage: kpasswd [OPTIONS]

Options:
  -a, --add-ssh-key <SSH_KEY>  Add an SSH public key (as it appears in ~/.ssh/authorized_keys)
  -l, --list-ssh-keys          List all SSH keys
  -d, --delete-ssh-key <TAG>   Delete an SSH key with the given tag/description
  -h, --help                   Print help
  -V, --version                Print version
```

## Security Notes

- The binary should have the SGID bit set to allow password changing without giving the calling user access to the privileged credentials.
- The configuration file should be readable only by root and the privileged group (shadow).
