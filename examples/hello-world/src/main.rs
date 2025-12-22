#![allow(clippy::all)]
#![allow(warnings)]

use std::net::SocketAddr;

use bytes::Bytes;
use futures::Future;
use http_body_util::Full;
use hyper::{Method, StatusCode};
use hyper::{server::conn::http1, service::service_fn};
use monoio::net::TcpListener;

use monet::{MapResponseLayer, Response, Router, State, extract::query::Query, get};

#[derive(Clone, Debug)]
struct AppState {
    data: String,
}

use rand::RngCore;
use rand::seq::IndexedMutRandom;
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

async fn merge1() -> &'static str {
    "merge 1"
}

async fn merge2() -> &'static str {
    "merge 2"
}

async fn dont_worry() -> &'static str {
    "No man land"
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

    let dummy_middleware = MapResponseLayer::new(|mut res: Response| -> Response {
        // If there is a content-length header, its value will be zero and axum will avoid
        // overwriting it. But this means our content-length doesn’t match the length of the
        // body, which leads to panics in Hyper. Thus we have to ensure that axum doesn’t add
        // on content-length headers until after middleware has been run.
        assert!(!res.headers().contains_key("content-length"));

        let mut rng = rand::rng();
        let mut nums: Vec<i32> = (1..3).collect();
        // And take a random pick (yes, we didn't need to shuffle first!):
        let num = nums.choose_mut(&mut rng).unwrap();
        if (*num == 2) {
            *res.body_mut() = "hijacked…\n".into();
        }
        res
    });

    let user_routes = Router::new().route("/users", get(merge1));

    let team_routes = Router::new().route("/teams", get(merge2));

    let app = Router::new()
        .route("/", get(root))
        .route("/sub", get(sub))
        .merge(user_routes)
        .merge(team_routes)
        .fallback(dont_worry)
        // .layer(dummy_middleware)
        .with_state(AppState {
            data: "no arc".to_string(),
        });
    monet::serve(listener, app).await;
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
