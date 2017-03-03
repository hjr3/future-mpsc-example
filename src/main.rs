#[macro_use] extern crate log;
extern crate env_logger;
extern crate futures;
extern crate tokio_core;

use std::{thread, time};

use futures::{Stream, Sink, Future};
use futures::sync::mpsc;

use tokio_core::reactor::Core;

#[derive(Debug)]
struct Stats {
    pub success: usize,
    pub failure: usize,
}

fn main() {

    env_logger::init().expect("Failed to initialize logger");

    // tokio Core is an event loop executor. An executor is what runs a future to
    // completion.
    let mut core = Core::new().expect("Failed to create core");

    // `core.remote()` is a thread safe version of `core.handle()`. Both `core.remote()` and
    // `core.handle()` are used to spawn a future. When a future is _spawned_, it basically means
    // that it is being executed.
    let remote = core.remote();

    // Now we create a multi-producer, single-consumer channel. This channel is very similar to the
    // mpsc channel in the std library. One big difference with this channel is that `tx` and `rx`
    // return futures. In order to have `tx` or `rx` actually do any work, they have to be
    // _executed_ by Core.
    //
    // The parameter passed to `mpsc::channel()` determines how large the queue is _per tx_. Since
    // we are cloning `tx` per iteration of the loop, we are guranteed 1 spot for each loop
    // iteration. Cloning tx is how we get multiple producers.
    //
    // For even more detail on mpsc, see https://tokio.rs/docs/going-deeper/synchronization/
    //
    // Quick note:
    //    - `tx` is of type `Sink`. A sink is something that you can place a value into and then
    //    _flush_ the value into the queue.
    //    - `rx` is of type `Stream`. A stream is an iterator of _future_ values.
    // More details on `tx` and `rx` below. For even more detail, see
    // https://tokio.rs/docs/getting-started/streams-and-sinks/
    let (tx, rx) = mpsc::channel(1);

    // Create a thread that performs some work.
    thread::spawn(move || {
        loop {
            let tx = tx.clone();

            // INSERT SOME FAKE WORK HERE - the work should be modeled as having a _future_ result.
            let delay = time::Duration::from_secs(1);
            thread::sleep(delay);

            // In this fake example, we do not care about the values of the `Ok` and `Err`
            // variants. thus, we can use `()` for both.
            // Note: `::futures::done()` will be called ::futures::result() in later versions of
            // the future crate.
            let f = ::futures::done::<(), ()>(Ok(()));
            // END FAKE WORK

            // `remote.spawn` accepts a closure with a single parameter of type `&Handle`. In this
            // example, the `&Handle` is not needed. The future returned from the closure will
            // executed.
            //
            // Note: We must use `remote.spawn()` instead of `handle.spawn()` because the Core was
            // created on a different thread.
            remote.spawn(|_| {

                // Use the `.then()` combinator to get the result of our "fake work" so we can send
                // it through the channel.
                f.then(|res| {

                    // Using `tx`, the result of the above work can be sent over the channel. Note
                    // that we also add the `.then()` combinator. Any future passed to
                    // `handle.spawn()` must be of type `Future<Item=(), Error=()>`. In the case of
                    // `tx.send()`, the `tx` (Sink) will be returned if the result was successfully
                    // flushed or a `SinkError` if the result could not be flushed.
                    tx
                    .send(res)
                    .then(|tx| {
                        match tx {
                            Ok(_tx) => {
                                info!("Sink flushed");
                                Ok(())
                            }
                            Err(e) => {
                                error!("Sink failed! {:?}", e);
                                Err(())
                            }
                        }
                    }) // <-- no semi-colon here! The result of tx.send.then() is a future.
                }) // <-- no semi-colon here! The result of `f.then()` will be spawned.
            });
        }
    });

    // I created a `Stats` type here. I could have use something like `counter: usize`, but that
    // implements `Copy`. I dislike examples that use types that implement `Copy` because they are
    // deceptively easier to make work.
    let mut stats = Stats { success: 0, failure: 0 };

    // As mentioned above, rx is a stream. That means we are expecting multiple _future_ values.
    // Here we use `for_each` to yield each value as it comes through the channel.
    let f2 = rx.for_each(|res| {

        // Remember that our fake work as modeled as `::futures::result()`. We need to check if the
        // future returned the `Ok` or `Err` variant and increment the counter accordingly.
        match res {
            Ok(_) => stats.success += 1,
            Err(_) => stats.failure += 1,
        }
        info!("stats = {:?}", stats);

        // The stream will stop on `Err`, so we need to return `Ok`.
        Ok(())
    });

    // The executor is started by the call to `core.run()` and will finish once the `f2` future is
    // finished. Keep in mind that since `rx` is a stream, it will not finish until there is an
    // error. Using a stream with `core.run()` is a common pattern and is how servers are normally
    // implemented.
    core.run(f2).expect("Core failed to run");
}
