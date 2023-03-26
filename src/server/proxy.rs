use bytes::Bytes;
use std::net::SocketAddr;
use tokio::net::{TcpListener, TcpStream};
use hyper::{server::conn::http1, Method};
use hyper::server::conn::http1::Builder;
use hyper::service::service_fn;
use hyper::body;
use http_body_util::{combinators::BoxBody, BodyExt, Empty};
use std::str::FromStr;
use hyper::{Request, Response};

pub async fn run_proxy(
    listener_addr: SocketAddr,
    proxy_pass: SocketAddr,
) {
    let listener = TcpListener::bind(listener_addr).await.unwrap();
    
    loop {
        let (stream, _) = listener.accept().await.unwrap();

        tokio::task::spawn(async move {
            if let Err(e) = Builder::new()
                .preserve_header_case(true)
                .title_case_headers(true)
                .serve_connection(
                    stream,
                service_fn( |mut req| proxy(req, proxy_pass)))
              //  .with_upgrades()
                .await
            {
               eprintln!("server error: {}", e);
            }
                
        });  
    }
}

async fn proxy(req: Request<hyper::body::Incoming>, proxy_pass: SocketAddr) -> Result<Response<BoxBody<Bytes, hyper::Error>>, hyper::Error> {
    println!("req: {:?}", req);

   
    // Read the Proxy Protocol header from the incoming request
    let mut conn_info = conn_info(req);
    let mut origin_host = origin_host(req);

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

    let uri_string = format!(
        "http://{}{}",
        out_addr_clone,
        req.uri()
        .path_and_query()
        .map(|x| x.as_str())
        .unwrap_or("/")
    );
    
    let uri = uri_string.parse().unwrap();
    *req.uri_mut() = uri;

    if let Some(host) = origin_host {
        req.headers_mut().insert("host", host.parse().unwrap());
    }

    upstream(req, proxy_pass); 
    
    Ok(Response::new(Empty::<Bytes>::new().map_err(|never| match never {}).boxed()))
}

fn conn_info<A>(req: Request<A>) -> Option<String> {
    return get_header(req, "PROXY");
}

fn origin_host<A>(req: Request<A>) -> Option<String> {
    return get_header(req, "host");
}

fn get_header<A>(req: Request<A>, header_name: &str) -> Opiton<String> {
    return req.headers().get(header_name).map(|value| String::from_utf8_lossy(value.as_bytes()).trim().to_string());
}


async fn upstream(req: Request<hyper::body::Incoming> , addr: SocketAddr) {
    
    let mut upstream = TcpStream::connect(addr).await.unwrap();

    let (mut sender, conn) = hyper::client::conn::http1::handshake(upstream).await.unwrap();

    tokio::task::spawn(async move {
        if let Err(e) = conn.await {
            eprintln!("Upstream connection failed: {:?}", e);
        }
    });

    sender.send_request(req).await
}
