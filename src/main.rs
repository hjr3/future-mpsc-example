#[macro_use] extern crate log;
extern crate env_logger;
extern crate futures;
extern crate tokio_core;

use std::{thread, time};

use futures::{Stream, Sink, Future};
use futures::sync::mpsc;

use tokio_core::reactor::Core;

#[derive(Debug)]
enum ResponseResult {
    Success,
    Failure
}

#[derive(Debug)]
struct Stats {
    pub success: usize,
    pub failure: usize,
}

fn main() {

    let mut core = Core::new().unwrap();
    let remote = core.remote();
    let (tx, rx) = mpsc::channel(1);

    thread::spawn(move || {
        loop {
            let tx = tx.clone();
            let delay = time::Duration::from_secs(1);
            thread::sleep(delay);

            remote.spawn(|handle| {
                handle.spawn(
                    tx
                    .send(ResponseResult::Success)
                    .then(|_| Ok(())) // spawn expects Ok(())
                );
                Ok(())
            });
        }
    });

    let mut stats = Stats { success: 0, failure: 0 };
    let f = rx.for_each(|res| {
        match res {
            ResponseResult::Success => stats.success += 1,
            ResponseResult::Failure => stats.failure += 1,
        }
        println!("stats = {:?}", stats);
        Ok(())
    });
    core.run(f).expect("Core failed to run");
}
