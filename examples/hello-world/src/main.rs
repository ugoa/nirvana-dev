#![allow(clippy::all)]
#![allow(warnings)]

use std::net::SocketAddr;

use bytes::Bytes;
use futures::Future;
use http_body_util::Full;
use hyper::{Method, Request, Response, StatusCode};
use hyper::{server::conn::http1, service::service_fn};
use monoio::net::TcpListener;

use nirvana::routing::router::Router;
use nirvana::{State, extract::query::Query, get};

#[derive(Clone, Debug)]
struct AppState {
    data: String,
}

use serde::Deserialize;

#[derive(Deserialize, Debug)]
struct DummyParams {
    a: usize,
    b: usize,
}

// fn main_old() {
//     let body = async {
//         println!("Running http server on 0.0.0.0:9527");
//         let addr: SocketAddr = ([0, 0, 0, 0], 9527).into();
//         let listener = TcpListener::bind(addr).unwrap();
//         let app = Router::new()
//             .route("/", get(root))
//             .route("/sub", get(sub))
//             .with_state(AppState {
//                 data: "no arc".to_string(),
//             });
//         nirvana::serve(listener, app).await;
//     };
//
//     monoio::RuntimeBuilder::<monoio::IoUringDriver>::new()
//         .enable_timer()
//         .build()
//         .expect("Failed building the Runtime")
//         .block_on(body);
// }

async fn root() -> &'static str {
    "Hello Daisy"
}

async fn sub(
    Query(q): Query<DummyParams>,
    State(app_state): State<AppState>,
    m: http::Method,
    path: http::Uri,
) -> String {
    format!(
        "You `{:?}` query at path `{:?}` has param {:?}, with state {:?}\n",
        m, path, q, app_state
    )
}

#[monoio::main(threads = 4, timer_enabled = true)]
async fn main() {
    let thread_id = std::thread::current().id();
    println!("Starting Monoio application on thread: {thread_id:?}",);

    let addr: SocketAddr = ([0, 0, 0, 0], 9527).into();
    let listener = TcpListener::bind(addr).unwrap();
    let app = Router::new()
        .route("/", get(root))
        .route("/sub", get(sub))
        .with_state(AppState {
            data: "no arc".to_string(),
        });
    nirvana::serve(listener, app).await;
}

// fn main() {
//     let body = async {
//         let thread_id = std::thread::current().id();
//         println!("Starting Monoio application on thread: {thread_id:?}",);
//
//         let addr: SocketAddr = ([0, 0, 0, 0], 9527).into();
//         let listener = TcpListener::bind(addr).unwrap();
//         let app = Router::new()
//             .route("/", get(root))
//             .route("/sub", get(sub))
//             .with_state(AppState {
//                 data: "no arc".to_string(),
//             });
//         nirvana::serve(listener, app).await;
//     };
//     #[allow(clippy::needless_collect)]
//     let threads: Vec<_> = (1..4u32)
//         .map(|_| {
//             ::std::thread::spawn(|| {
//                 monoio::RuntimeBuilder::<monoio::FusionDriver>::new()
//                     .enable_timer()
//                     .build()
//                     .expect("Failed building the Runtime")
//                     .block_on(async {
//                         let thread_id = std::thread::current().id();
//                         println!("Starting Monoio application on thread: {thread_id:?}",);
//
//                         let addr: SocketAddr = ([0, 0, 0, 0], 9527).into();
//                         let listener = TcpListener::bind(addr).unwrap();
//                         let app = Router::new()
//                             .route("/", get(root))
//                             .route("/sub", get(sub))
//                             .with_state(AppState {
//                                 data: "no arc".to_string(),
//                             });
//                         nirvana::serve(listener, app).await;
//                     });
//             })
//         })
//         .collect();
//     monoio::RuntimeBuilder::<monoio::FusionDriver>::new()
//         .enable_timer()
//         .build()
//         .expect("Failed building the Runtime")
//         .block_on(body);
//     threads.into_iter().for_each(|t| {
//         let _ = t.join();
//     });
// }
