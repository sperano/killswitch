use clap::Parser;
use axum::{
    extract::{Request, State},
    http::{HeaderMap, StatusCode},
    middleware::{self, Next},
    response::Response,
    routing::{get, post},
    Router,
};
use std::net::SocketAddr;
use std::process::{Child, Command};
use std::sync::Arc;
use tokio::sync::Mutex;
use tracing::{error, info, warn};

#[derive(Parser, Debug)]
#[command(name = "killswitch")]
#[command(about = "A kill switch server", long_about = None)]
struct Args {
    /// Port to listen on
    #[arg(short = 'P', long, default_value_t = 9966)]
    port: u16,
    /// Command to execute
    #[arg(required = true, num_args = 1..)]
    command: Vec<String>,
}

fn launch_process(command: &[String]) -> std::result::Result<Child, std::io::Error> {
    info!("Launching process: {} {:?}", command[0], &command[1..]);
    Command::new(&command[0])
        .args(&command[1..])
        .spawn()
}

// Shared application state
struct SharedState {
    child: Option<Child>,
    command: Vec<String>,
    password: Option<String>,
}

type AppState = Arc<Mutex<SharedState>>;

// Middleware to check authentication
async fn check_auth(
    State(state): State<AppState>,
    headers: HeaderMap,
    request: Request,
    next: Next,
) -> Result<Response, StatusCode> {
    let state = state.lock().await;

    if let Some(required_password) = &state.password {
        let auth_header = headers
            .get("authorization")
            .and_then(|v| v.to_str().ok());

        match auth_header {
            Some(auth) if auth.starts_with("Bearer ") => {
                let token = &auth[7..]; // Skip "Bearer "
                if token == required_password {
                    drop(state); // Release lock before proceeding
                    Ok(next.run(request).await)
                } else {
                    let msg = "Invalid credentials";
                    error!("{}", msg);
                    Err(StatusCode::UNAUTHORIZED)
                }
            }
            _ => {
                let msg = "Missing or invalid Authorization header";
                error!("{}", msg);
                Err(StatusCode::UNAUTHORIZED)
            }
        }
    } else {
        drop(state); // Release lock before proceeding
        Ok(next.run(request).await)
    }
}

async fn start_handler(State(state): State<AppState>) -> (StatusCode, String) {
    let mut state = state.lock().await;

    if state.child.is_some() {
        let msg = "Process is already running";
        error!("{}", msg);
        return (StatusCode::CONFLICT, msg.to_string());
    }

    match launch_process(&state.command) {
        Ok(process) => {
            let pid = process.id();
            state.child = Some(process);
            let msg = format!("Process started with PID {}", pid);
            info!("{}", msg);
            (StatusCode::OK, msg)
        }
        Err(e) => {
            let msg = format!("Failed to start process: {}", e);
            error!("{}", msg);
            (StatusCode::INTERNAL_SERVER_ERROR, msg)
        }
    }
}

async fn stop_handler(State(state): State<AppState>) -> (StatusCode, String) {
    let mut state = state.lock().await;

    match state.child.as_mut() {
        Some(process) => {
            match kill_tree::blocking::kill_tree(process.id()) {
                Ok(_) => {
                    state.child = None;
                    let msg = "Process stopped";
                    info!("{}", msg);
                    (StatusCode::OK, msg.to_string())
                }
                Err(e) => {
                    let msg = format!("Failed to stop process: {}", e);
                    error!("{}", msg);
                    (StatusCode::INTERNAL_SERVER_ERROR, msg)
                }
            }
        }
        None => {
            let msg = "No process is running";
            warn!("{}", msg);
            (StatusCode::NOT_FOUND, msg.to_string())
        }
    }
}

async fn status_handler(State(state): State<AppState>) -> String {
    let state = state.lock().await;
    match state.child.as_ref() {
        Some(process) => {
            let msg = format!("Process is running with PID {}", process.id());
            info!("{}", msg);
            msg
        }
        None => {
            let msg = "No process is running";
            info!("{}", msg);
            msg.to_string()
        }
    }
}

#[tokio::main]
async fn main() {
    let args = Args::parse();

    tracing_subscriber::fmt()
        .with_target(true)
        .with_level(true)
        .init();

    // Read password from environment variable if set
    let password = std::env::var("KILLSWITCH_PASSWORD").ok();
    if password.is_some() {
        info!("Authentication enabled via KILLSWITCH_PASSWORD environment variable");
    } else {
        info!("No authentication configured");
    }

    let initial_child = launch_process(&args.command).expect("Failed to spawn process");
    info!("Process started with PID {}", initial_child.id());
    let shared_state: AppState = Arc::new(Mutex::new(SharedState {
        child: Some(initial_child),
        command: args.command,
        password,
    }));

    // Spawn a background thread to check if the process exits early
    let state_clone = Arc::clone(&shared_state);
    std::thread::spawn(move || {
        std::thread::sleep(std::time::Duration::from_secs(1));
        let mut state = state_clone.blocking_lock();
        if let Some(child) = state.child.as_mut() {
            match child.try_wait() {
                Ok(Some(status)) => {
                    error!("Process exited with status: {:?}", status);
                    std::process::exit(1);
                }
                Ok(None) => {
                    // Process is still running, nothing to do
                }
                Err(e) => {
                    error!("Error checking process status: {}", e);
                }
            }
        }
    });

    // Spawn a background thread to periodically check if child is still running
    let state_clone = Arc::clone(&shared_state);
    std::thread::spawn(move || {
        loop {
            std::thread::sleep(std::time::Duration::from_secs(5));
            let mut state = state_clone.blocking_lock();
            if let Some(child) = state.child.as_mut() {
                match child.try_wait() {
                    Ok(Some(status)) => {
                        warn!("Child process exited with status: {:?}", status);
                        state.child = None;
                    }
                    Ok(None) => {
                        // Process is still running
                    }
                    Err(e) => {
                        error!("Error checking child process status: {}", e);
                    }
                }
            }
        }
    });

    let app = Router::new()
        .route("/", get(|| async { "Hello, world!" }))
        .route("/status", get(status_handler))
        .route("/start", post(start_handler))
        .route("/stop", post(stop_handler))
        .route_layer(middleware::from_fn_with_state(
            Arc::clone(&shared_state),
            check_auth,
        ))
        .with_state(shared_state);

    let addr = SocketAddr::from(([0, 0, 0, 0], args.port));
    info!("Listening on http://{}", addr);
    axum::serve(tokio::net::TcpListener::bind(addr).await.unwrap(), app)
        .await
        .unwrap();
}
