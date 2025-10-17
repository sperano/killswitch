use axum::{routing::get, Router};
use std::net::SocketAddr;
use std::process::Command;

fn launch_process() -> std::process::Child {
    Command::new("ping")
        .arg("localhost")
        .spawn()                          
        .expect("Failed to spawn process")   
}

#[tokio::main]
async fn main() {
    let child = launch_process();
    let child_id = child.id();

    // Define routes
    let app = Router::new()
        .route("/", get(|| async { "Hello, world!" }))
        .route(
            "/status",
            get(move || {
                let pid = child_id; // copy u32 into async block
                async move { format!("process id is {}", pid) }
            }),
        );

    // Start server
    let addr = SocketAddr::from(([0, 0, 0, 0], 8080));
    println!("Listening on http://{}", addr);
    axum::serve(tokio::net::TcpListener::bind(addr).await.unwrap(), app)
        .await
        .unwrap();
}
