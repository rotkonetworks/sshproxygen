# sshproxygen
Very simple ssh jumper proxy generator. Useful if you have dynamic IP and
want to limit access to certain IP address f.g. your jumper server.

# Install
```
curl -L "https://github.com/rotkonetworks/sshproxygen/releases/latest/download/sshproxygen-$(uname -s | tr '[:upper:]' '[:lower:]')-amd64" -o sshproxygen && chmod +x sshproxygen && sudo mv sshproxygen /usr/local/bin/
```

# Usage
```
sudo sshproxygen add bkk10:proxyssh@172.16.10.1
```

# Build and install
```
# Build and install
cargo build --release
sudo install -m 0755 target/release/sshproxygen /usr/local/bin/

# Create initial config
cat > config.toml << EOF
ssh_key = "/etc/ssh/id_rsa"

[proxies]
bkk10 = { target = "172.16.10.1", port = 22 }
bkk20 = { target = "172.16.20.1", port = 22 }
EOF

# Install from config
sudo sshproxygen -c config.toml install

# Or add individual proxies
sudo sshproxygen add bkk10:proxyssh@172.16.10.1
```
