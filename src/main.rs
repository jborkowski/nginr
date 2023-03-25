use hyper::service::{make_service_fn, service_fn};
use hyper::{Body, Client, Request, Response, Server, Uri};
use std::net::SocketAddr;
use std::str::FromStr;
use tokio::sync::oneshot;
use tokio::task;

async fn handle_request(mut req: Request<Body>) -> Result<Response<Body>, hyper::Error> {
    // Read the Proxy Protocol header from the incoming request
    let mut conn_info = None;
    let mut origin_host = None;
    for (name, value) in req.headers() {
        if name == "PROXY" {
            let s = String::from_utf8_lossy(value.as_bytes()).trim().to_string();
            conn_info = Some(s);
            break;
        }
        if name == "host" {
            let s = String::from_utf8_lossy(value.as_bytes()).trim().to_string();
            origin_host = Some(s);
            break;
        }
    }

    // If the Proxy Protocol header was present, extract the client IP and port
    if let Some(conn_info) = conn_info {
        let parts: Vec<&str> = conn_info.split(' ').collect();
        if parts.len() >= 5 {
            let client_addr = SocketAddr::from_str(parts[2]).unwrap();
            req.headers_mut().insert(
                "X-Real-IP",
                format!("{}", client_addr.ip()).parse().unwrap(),
            );
            req.headers_mut().insert(
                "X-Forwarded-For",
                format!("{}", client_addr.ip()).parse().unwrap(),
            );
            req.headers_mut().insert(
                "X-Forwarded-Port",
                client_addr.port().to_string().parse().unwrap(),
            );
        }
    }

    // Create a new HTTP client
    let client = Client::new();

    // Build the URI for the target server
    let uri = format!("http://{}", req.uri().host().unwrap())
        .parse::<Uri>()
        .unwrap();

    // Build the new request to forward to the target server
    let mut new_req = Request::builder()
        .method(req.method())
        .uri(uri)
        .version(req.version())
        .body(req.into_body())
        .unwrap();

    // Set the original host header to the new request
    if let Some(host) = origin_host {
        new_req.headers_mut().insert("host", host.parse().unwrap());
    }

    let res = client.request(new_req).await?;

    Ok(res)
}

async fn run_server(addr: SocketAddr, tx: oneshot::Sender<()>) {
    let make_svc =
        make_service_fn(|_conn| async { Ok::<_, hyper::Error>(service_fn(handle_request)) });

    // Create a new server listening on the specified address
    let server = Server::bind(&addr).serve(make_svc);

    // Start the server
    let graceful = server.with_graceful_shutdown(async {
        tx.send(()).unwrap();
    });
    if let Err(e) = graceful.await {
        eprintln!("server error: {}", e);
    }
}

#[tokio::main]
async fn main() {
    let address = "127.0.0.1:8080".parse().unwrap();

    let (tx, rx) = oneshot::channel();

    let server_task = task::spawn(run_server(address, tx));

    let _ = rx.await;

    server_task.abort();
}
                      