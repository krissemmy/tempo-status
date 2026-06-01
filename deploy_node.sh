#!/bin/bash
sudo apt-get update && sudo apt-get upgrade -y

adduser node

usermod -aG sudo node

su - node

# Add ssh access for node user
# sudo nano /etc/ssh/sshd_config

sudo echo 'PasswordAuthentication yes
KbdInteractiveAuthentication yes
UsePAM yes' | sudo tee -a /etc/ssh/sshd_config >/dev/null


sudo systemctl restart ssh

# Exit and login as node user

ssh node@<ip_address>

# Check OS version
cat /etc/os-release

# Check glibc version
ldd --version | head -n 1



sudo apt-get update && sudo apt-get upgrade -y
sudo apt-get install ccze build-essential pkg-config libssl-dev -y

# Install Prometheus
# Visit https://prometheus.io/download/ for the latest version
PROM_VERSION=$(curl -s https://api.github.com/repos/prometheus/prometheus/releases/latest | grep tag_name | cut -d '"' -f 4 | cut -c 2-)
wget https://github.com/prometheus/prometheus/releases/download/v${PROM_VERSION}/prometheus-${PROM_VERSION}.linux-amd64.tar.gz
tar xvfz prometheus-*.tar.gz
rm prometheus-*.tar.gz
# cd prometheus-*

#Create a file for Prometheus
sudo tee /etc/systemd/system/prometheus.service > /dev/null <<EOF
[Unit]
Description=Prometheus
Wants=network-online.target
After=network-online.target

[Service]
User=node
Type=simple
WorkingDirectory=/home/node/prometheus-${PROM_VERSION}.linux-amd64
ExecStart=/home/node/prometheus-${PROM_VERSION}.linux-amd64/prometheus \
  --config.file=prometheus.yml

[Install]
WantedBy=multi-user.target
EOF

echo '  - job_name: "tempo"
    static_configs:
      - targets: ["127.0.0.1:9000"]' | sudo tee -a /home/node/prometheus-${PROM_VERSION}.linux-amd64/prometheus.yml >/dev/null

sudo systemctl enable prometheus
sudo systemctl start prometheus

# Install Grafana
sudo mkdir -p /etc/apt/keyrings

curl -fsSL https://apt.grafana.com/gpg.key | sudo gpg --dearmor -o /etc/apt/keyrings/grafana.gpg

echo "deb [signed-by=/etc/apt/keyrings/grafana.gpg] https://apt.grafana.com stable main" \
  | sudo tee /etc/apt/sources.list.d/grafana.list > /dev/null
sudo apt-get update
sudo apt-get install -y grafana

sudo systemctl enable --now grafana-server
sudo systemctl status grafana-server --no-pager



# Install Tempo
curl -L https://tempo.xyz/install | bash
source /home/node/.bashrc
tempo --version

mkdir -p tempo-data

tempo download --datadir tempo-data/
# Start Tempo node
tempo node -vvv \
    --follow \
    --datadir tempo-data/ \
    --port 30303 \
    --discovery.addr 0.0.0.0 \
    --discovery.port 30303 \
    --http \
    --http.addr 0.0.0.0 \
    --http.port 8545 \
    --http.api eth,net,web3,txpool,trace \
    --metrics 9000


sudo tee /etc/systemd/system/tempo.service > /dev/null <<EOF
[Unit]
Description=Tempo RPC Node
After=network.target
Wants=network.target
 
[Service]
Type=simple
User=node
Group=node
Environment=RUST_LOG=info
WorkingDirectory=$HOME
ExecStart=/home/node/.tempo/bin/tempo node \\
  --datadir /home/node/tempo-data/  \\
  --follow \\
  --port 30303 \\
  --discovery.addr 0.0.0.0 \\
  --discovery.port 30303 \\
  --http \\
  --http.addr 0.0.0.0 \\
  --http.port 8545 \\
  --http.api eth,net,web3,txpool,trace \\
  --metrics 9000 \\
 
Restart=always
RestartSec=10
StandardOutput=journal
StandardError=journal
SyslogIdentifier=tempo
LimitNOFILE=infinity
 
[Install]
WantedBy=multi-user.target
EOF
 
# Enable and start
sudo systemctl daemon-reload
sudo systemctl enable tempo
sudo systemctl start tempo
 
sudo journalctl -fu tempo | ccze


# Create status page
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
source $HOME/.cargo/env

mkdir -p tempo-status/
cd tempo-status/
cargo init --bin

export NODE_RPC="http://127.0.0.1:8545"
export REF_RPC="https://rpc.testnet.tempo.xyz"
export MAX_LAG="3"
export BIND="0.0.0.0:8080"

cargo run --release


# Check ports
ss -lntup


# Check Tempo logs
sudo journalctl -fu tempo | ccze


test11

curl  -H 'Content-Type: application/json' --data '{
            "id": 1,
            "jsonrpc": "2.0",
            "method": "eth_blockNumber",
            "params": []
      }' 'http://127.0.0.1'


curl  -H 'Content-Type: application/json' --data '{
            "id": 1,
            "jsonrpc": "2.0",
            "method": "eth_syncing",
            "params": []
      }' 'http://127.0.0.1:8545'


# node@enormous-aphid:~$ tempo download --datadir tempo-data/
# 2025-12-24T03:43:49.887147Z  INFO Initialized tracing, debug log directory: /home/node/.cache/reth/logs/tempo-testnet

# https://medium.com/@krissemmy17/how-to-run-a-tempo-testnet-rpc-node-on-ubuntu-with-monitoring-d6447fce197f