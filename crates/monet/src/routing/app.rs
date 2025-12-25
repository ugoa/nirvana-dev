use std::convert::Infallible;

use tower::{ServiceBuilder, layer::util::Identity};

pub struct App<L = Identity> {
    pub routes: Vec<ServiceBuilder<L>>,
}

impl App {
    fn new() -> Self {
        Self {
            routes: vec![ServiceBuilder::default()],
        }
    }
}
