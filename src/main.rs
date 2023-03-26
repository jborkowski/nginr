mod server;
use server::proxy::run_proxy;

#[tokio::main]
async fn main() {
    let address = "127.0.0.1:8080".parse().unwrap();

    let server1 = run_proxy(address, address);

    let _ = server1.await;
}
                      
