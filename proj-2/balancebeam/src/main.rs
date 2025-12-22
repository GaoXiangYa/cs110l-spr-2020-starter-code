mod request;
mod response;

use clap::Parser;
use rand::{Rng, SeedableRng};
use std::collections::{HashMap, VecDeque};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::net::TcpListener;
use tokio::net::TcpStream;
use tokio::sync::Mutex;
use tokio::sync::RwLock;
use tokio::time::sleep;

/// Contains information parsed from the command-line invocation of balancebeam. The Clap macros
/// provide a fancy way to automatically construct a command-line argument parser.
#[derive(Parser, Debug)]
#[command(about = "Fun with load balancing")]
struct CmdOptions {
    /// "IP/port to bind to"
    #[arg(short, long, default_value = "0.0.0.0:1100")]
    bind: String,
    /// "Upstream host to forward requests to"
    #[arg(short, long)]
    upstream: Vec<String>,
    /// "Perform active health checks on this interval (in seconds)"
    #[arg(long, default_value = "10")]
    active_health_check_interval: usize,
    /// "Path to send request to for active health checks"
    #[arg(long, default_value = "/")]
    active_health_check_path: String,
    /// "Maximum number of requests to accept per IP per minute (0 = unlimited)"
    #[arg(long, default_value = "0")]
    max_requests_per_minute: usize,
}

/// Contains information about the state of balancebeam (e.g. what servers we are currently proxying
/// to, what servers have failed, rate limiting counts, etc.)
///
/// You should add fields to this struct in later milestones.
#[derive(Clone)]
struct ProxyState {
    /// How frequently we check whether upstream servers are alive (Milestone 4)
    active_health_check_interval: usize,
    /// Where we should send requests when doing active health checks (Milestone 4)
    active_health_check_path: String,
    /// Maximum number of requests an individual IP can make in a minute (Milestone 5)
    #[allow(dead_code)]
    max_requests_per_minute: usize,
    /// Addresses of servers that we are proxying to
    upstream_addresses: Vec<String>,
    /// Active servers
    active_upstream_addresses: Arc<RwLock<Vec<String>>>,
    request_state: Arc<Mutex<HashMap<String, VecDeque<Instant>>>>,
}

#[tokio::main]
async fn main() {
    // Initialize the logging library. You can print log messages using the `log` macros:
    // https://docs.rs/log/0.4.8/log/ You are welcome to continue using print! statements; this
    // just looks a little prettier.
    if let Err(_) = std::env::var("RUST_LOG") {
        std::env::set_var("RUST_LOG", "debug");
    }
    pretty_env_logger::init();

    // Parse the command line arguments passed to this program
    let options = CmdOptions::parse();
    if options.upstream.len() < 1 {
        log::error!("At least one upstream server must be specified using the --upstream option.");
        std::process::exit(1);
    }

    // Start listening for connections
    let listener = match TcpListener::bind(&options.bind).await {
        Ok(listener) => listener,
        Err(err) => {
            log::error!("Could not bind to {}: {}", options.bind, err);
            std::process::exit(1);
        }
    };
    log::info!("Listening for requests on {}", options.bind);

    // Handle incoming connections
    let state = Arc::new(ProxyState {
        upstream_addresses: options.upstream,
        active_health_check_interval: options.active_health_check_interval,
        active_health_check_path: options.active_health_check_path,
        max_requests_per_minute: options.max_requests_per_minute,
        active_upstream_addresses: Arc::new(RwLock::new(Vec::new())),
        request_state: Arc::new(Mutex::new(HashMap::new())),
    });

    if !state.active_health_check_path.is_empty() {
        log::info!("Starting health check task");
        log::info!(
            "health check interval {}",
            state.active_health_check_interval
        );
        let health_check_state = state.clone();
        tokio::spawn(async move {
            health_check(health_check_state).await;
        });
    }

    log::info!("Starting to accept connections");
    while let Ok((stream, _socked_addr)) = listener.accept().await {
        let shared_state = state.clone();
        tokio::spawn(async move {
            handle_connection(stream, shared_state).await;
        });
    }
}

async fn health_check(state: Arc<ProxyState>) {
    loop {
        log::info!("Starting health check cycle");
        sleep(Duration::from_secs(
            state.active_health_check_interval.try_into().unwrap(),
        ))
        .await;
        let mut active_upstream_addresses = state.active_upstream_addresses.write().await;
        active_upstream_addresses.clear();

        for upstream_addr in state.upstream_addresses.iter() {
            let request = http::Request::builder()
                .method(http::Method::GET)
                .uri(&state.active_health_check_path)
                .header("Host", upstream_addr)
                .body(Vec::<u8>::new())
                .expect("build http::Request failed!");

            match TcpStream::connect(upstream_addr).await {
                Ok(mut stream) => {
                    if let Err(e) = request::write_to_stream(&request, &mut stream).await {
                        log::warn!("Health check request to {} failed: {}", upstream_addr, e);
                        return;
                    }
                    let response = response::read_from_stream(&mut stream, request.method()).await;
                    match response {
                        Ok(resp) => {
                            if resp.status() == http::StatusCode::OK {
                                log::info!("Upstream {} is healthy", upstream_addr);
                                active_upstream_addresses.push(upstream_addr.clone());
                            } else {
                                log::warn!(
                                    "Upstream {} returned status code {}",
                                    upstream_addr,
                                    resp.status()
                                );
                            }
                        }
                        Err(_) => {
                            log::warn!("Health check response from {} failed", upstream_addr);
                        }
                    }
                }
                Err(err) => {
                    log::warn!("Could not connect to {}: {}", upstream_addr, err);
                    continue;
                }
            }
        }

        log::info!(
            "Health check complete: {} active upstream servers",
            active_upstream_addresses.len()
        );
    }
}

async fn read_upstream_addresses(state: &Arc<ProxyState>) -> (usize, String) {
    let read_lock = state.active_upstream_addresses.read().await;
    let mut rng = rand::rngs::StdRng::from_entropy();
    let upstream_idx = rng.gen_range(0..read_lock.len());
    let upstream_ip = read_lock[upstream_idx].clone();
    (upstream_idx, upstream_ip)
}

async fn delete_upstream_address(state: &Arc<ProxyState>, upstream_idx: usize) {
    let mut write_lock = state.active_upstream_addresses.write().await;
    if upstream_idx < write_lock.len() {
        log::info!(
            "Upstream {} is down, removed from upstream list\n",
            upstream_idx
        );
        write_lock.remove(upstream_idx);
    }
}

async fn add_upstream_address(state: &Arc<ProxyState>, upstream_ip: String) {
    let mut write_lock = state.active_upstream_addresses.write().await;
    log::info!("Pick activate upstream {}\n", upstream_ip);
    if !write_lock.contains(&upstream_ip) {
        write_lock.push(upstream_ip);
    }
}

async fn connect_to_upstream(state: Arc<ProxyState>) -> Result<TcpStream, std::io::Error> {
    loop {
        if state.active_upstream_addresses.read().await.len() == 0 {
            log::error!("No active upstream servers available");
            sleep(Duration::from_secs(3)).await;
            continue;
        }
        let (upstream_idx, mut upstream_ip) = read_upstream_addresses(&state).await;
        log::debug!("Connecting to upstream {}", upstream_ip);
        // TODO: implement failover (milestone 3)
        let stream = TcpStream::connect(upstream_ip).await;
        let ret = match stream {
            Ok(stream) => stream,
            Err(_) => {
                delete_upstream_address(&state, upstream_idx).await;

                (_, upstream_ip) = read_upstream_addresses(&state).await;

                add_upstream_address(&state, upstream_ip.clone()).await;
                let new_stream = TcpStream::connect(&upstream_ip).await?;
                new_stream
            }
        };

        return Ok(ret);
    }
}

async fn send_response(client_conn: &mut TcpStream, response: &http::Response<Vec<u8>>) {
    let client_ip = client_conn.peer_addr().unwrap().ip().to_string();
    log::info!(
        "{} <- {}",
        client_ip,
        response::format_response_line(&response)
    );
    if let Err(error) = response::write_to_stream(&response, client_conn).await {
        log::warn!("Failed to send response to client: {}", error);
        return;
    }
}


async fn handle_connection(mut client_conn: TcpStream, state: Arc<ProxyState>) {
    let client_ip = client_conn.peer_addr().unwrap().ip().to_string();
    log::info!("Connection received from {}", client_ip);

    // Open a connection to a random destination server
    let mut upstream_conn = match connect_to_upstream(state.clone()).await {
        Ok(stream) => stream,
        Err(_error) => {
            // connect_to_upstream(state).await?
            let response = response::make_http_error(http::StatusCode::BAD_GATEWAY);
            // current stream is died we need to choose another upstream
            log::debug!("Failed to connect to upstream server");
            send_response(&mut client_conn, &response).await;
            return;
        }
    };
    let upstream_ip = upstream_conn.peer_addr().unwrap().ip().to_string();

    // The client may now send us one or more requests. Keep trying to read requests until the
    // client hangs up or we get an error.
    loop {
        // Read a request from the client
        let mut request = match request::read_from_stream(&mut client_conn).await {
            Ok(request) => request,
            // Handle case where client closed connection and is no longer sending requests
            Err(request::Error::IncompleteRequest(0)) => {
                log::debug!("Client finished sending requests. Shutting down connection");
                return;
            }
            // Handle I/O error in reading from the client
            Err(request::Error::ConnectionError(io_err)) => {
                log::info!("Error reading request from client stream: {}", io_err);
                return;
            }
            Err(error) => {
                log::debug!("Error parsing request: {:?}", error);
                let response = response::make_http_error(match error {
                    request::Error::IncompleteRequest(_)
                    | request::Error::MalformedRequest(_)
                    | request::Error::InvalidContentLength
                    | request::Error::ContentLengthMismatch => http::StatusCode::BAD_REQUEST,
                    request::Error::RequestBodyTooLarge => http::StatusCode::PAYLOAD_TOO_LARGE,
                    request::Error::ConnectionError(_) => http::StatusCode::SERVICE_UNAVAILABLE,
                });
                send_response(&mut client_conn, &response).await;
                continue;
            }
        };
        log::info!(
            "{} -> {}: {}",
            client_ip,
            upstream_ip,
            request::format_request_line(&request)
        );

        if state.max_requests_per_minute != 0 {
            let now = Instant::now();
            let should_reject = {
                let mut stats = state.request_state.lock().await;
                let entry = stats.entry(client_ip.clone()).or_insert_with(VecDeque::new);

                while let Some(ts) = entry.front() {
                    if now.duration_since(*ts) > Duration::from_secs(60) {
                        entry.pop_front();
                    } else {
                        break;
                    }
                }

                if entry.len() >= state.max_requests_per_minute {
                    log::debug!(
                        "sliding windows len = {}, max_requests_per_minute = {}",
                        entry.len(),
                        state.max_requests_per_minute
                    );
                    true
                } else {
                    entry.push_back(now);
                    false
                }
            };

            if should_reject {
                let response = response::make_http_error(http::StatusCode::TOO_MANY_REQUESTS);
                send_response(&mut client_conn, &response).await;
                continue;
            }
        }

        // Add X-Forwarded-For header so that the upstream server knows the client's IP address.
        // (We're the ones connecting directly to the upstream server, so without this header, the
        // upstream server will only know our IP, not the client's.)
        request::extend_header_value(&mut request, "x-forwarded-for", &client_ip);

        // Forward the request to the server
        if let Err(error) = request::write_to_stream(&request, &mut upstream_conn).await {
            log::error!(
                "Failed to send request to upstream {}: {}",
                upstream_ip,
                error
            );
            let response = response::make_http_error(http::StatusCode::BAD_GATEWAY);
            send_response(&mut client_conn, &response).await;
            return;
        }
        log::debug!("Forwarded request to server");

        // Read the server's response
        let response = match response::read_from_stream(&mut upstream_conn, request.method()).await
        {
            Ok(response) => response,
            Err(error) => {
                log::error!("Error reading response from server: {:?}", error);
                let response = response::make_http_error(http::StatusCode::BAD_GATEWAY);
                send_response(&mut client_conn, &response).await;
                return;
            }
        };
        // Forward the response to the client
        send_response(&mut client_conn, &response).await;
        log::debug!("Forwarded response to client");
    }
}
