use log::{error, info};
use rand::Rng;
use serde_json::json;
use std::collections::HashMap;
use std::io::{self, Write};
use std::net::{TcpListener, TcpStream};
use std::process::Stdio;
use std::sync::Arc;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader as AsyncBufReader};
use tokio::process::Command as TokioCommand;
use tokio::sync::{watch, Mutex as TokioMutex};

pub fn generate_random_port() -> u16 {
    let mut rng = rand::thread_rng();
    loop {
        let port: u16 = rng.gen_range(1024..=65535);
        if is_port_available(port) {
            info!("Using port {}", port);
            return port;
        }
    }
}

fn is_port_available(port: u16) -> bool {
    TcpListener::bind(("127.0.0.1", port)).is_ok()
}

pub async fn spawn_rdbg(port: u16, tx: watch::Sender<bool>) -> Result<(), io::Error> {
    let args = format!("--open --port {}", port);

    let mut envs: HashMap<String, String> = HashMap::new();
    envs.insert("DEBUG_DAP_SHOW_PROTOCOL".to_string(), "1".to_string());

    let mut cmd = TokioCommand::new("rdbg");

    cmd.args(args.split_whitespace())
        .envs(&envs)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());

    info!(
        "Spawn rdbg and listen on port {} args: {:?} env: {:?}",
        port, args, envs
    );

    let mut child = cmd.spawn()?;

    let stdin_handle = Arc::new(TokioMutex::new(
        child.stdin.take().expect("Failed to open stdin"),
    ));

    // Capture and log stdout output
    if let Some(stdout) = child.stdout.take() {
        let mut stdout_reader = AsyncBufReader::new(stdout).lines();
        tokio::spawn(async move {
            while let Some(line) = stdout_reader.next_line().await.unwrap_or(None) {
                info!("STDOUT: {}", line);
            }
        });
    }

    // Capture and log stderr output
    if let Some(stderr) = child.stderr.take() {
        let mut stderr_reader = AsyncBufReader::new(stderr).lines();
        let tx = tx.clone();
        tokio::spawn(async move {
            while let Some(line) = stderr_reader.next_line().await.unwrap_or(None) {
                info!("STDERR: {}", line);
                if line.contains("DEBUGGER: wait for debugger connection...") {
                    info!("rdbg is ready");
                    let _ = tx.send(true);
                }
            }
        });
    }

    // Spawn a task to wait for the child process to exit
    tokio::spawn(async move {
        let status = child.wait().await;
        match status {
            Ok(status) => info!("rdbg exited with status: {}", status),
            Err(e) => error!("rdbg exited with error: {}", e),
        }
    });

    // Write empty line to stdin so rdbg won't immediately exit
    let _stdin = Arc::clone(&stdin_handle);
    tokio::spawn(async move {
        let mut stdin = _stdin.lock().await;
        if let Err(e) = stdin.write_all(b"i\n").await {
            error!("Failed to write to stdin: {}", e);
        }
        if let Err(e) = stdin.flush().await {
            error!("Failed to flush stdin: {}", e);
        }
    });

    Ok(())
}

pub fn connect_to_port(port: u16) -> Result<(), io::Error> {
    let address = format!("127.0.0.1:{}", port);
    {
        let mut stream = TcpStream::connect(&address)?;

        // Create the DAP initialize request message
        let initialize_request = json!({
            "type": "request",
            "seq": 1,
            "command": "initialize",
            "arguments": {
                "clientID": "example-client",
                "clientName": "Example Client",
                "adapterID": "example-adapter",
                "pathFormat": "path",
                "linesStartAt1": true,
                "columnsStartAt1": true,
                "supportsVariableType": true,
                "supportsVariablePaging": true,
                "supportsRunInTerminalRequest": true,
                "locale": "en-us"
            }
        });

        // Convert the message to a JSON string
        let message = initialize_request.to_string();

        // Send the message length followed by the message itself
        let message_length = message.len();
        let header = format!("Content-Length: {}\r\n\r\n", message_length);

        info!("DAP initialization request: {} {:?}", message, header);

        stream.write_all(header.as_bytes())?;
        stream.write_all(message.as_bytes())?;

        info!("DAP initialization request sent successfully.")
    }

    Ok(())
}
