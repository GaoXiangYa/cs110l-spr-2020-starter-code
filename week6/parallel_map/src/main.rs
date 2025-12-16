use crossbeam_channel::{self, unbounded, Receiver, Sender};
use std::{process::id, thread, time};

fn parallel_map<T, U, F>(mut input_vec: Vec<T>, num_threads: usize, f: F) -> Vec<U>
where
    F: FnOnce(T) -> U + Send + Copy + 'static,
    T: Send + 'static,
    U: Send + 'static + Default,
{
    let mut output_vec: Vec<U> = Vec::with_capacity(input_vec.len());
    // TODO: implement parallel map!
    let (tx1, rx1): (Sender<U>, Receiver<U>) = unbounded();
    let (tx2, rx2): (Sender<U>, Receiver<U>) = unbounded();

    for val in input_vec.into_iter() {
        tx1.send(f(val))
            .expect("tx1 send message failed!");
    }

    drop(tx1);

    let mut threads = Vec::new();
    for _ in 0..num_threads {
        let recv = rx1.clone();
        let sender = tx2.clone();
        threads.push(thread::spawn(move || {
            while let Ok(num) = recv.recv() {
                sender.send(num).expect("tx2 send message failed");
            }
        }));
    }

    drop(tx2);

    while let Ok(num) = rx2.recv() {
        output_vec.push(num);
    }

    for t in threads {
        t.join().expect("panic in thread");
    }

    output_vec
}

fn main() {
    let v = vec![6, 7, 8, 9, 10, 1, 2, 3, 4, 5, 12, 18, 11, 5, 20];
    let squares = parallel_map(v, 10, |num| {
        println!("{} squared is {}", num, num * num);
        thread::sleep(time::Duration::from_millis(500));
        num * num
    });
    println!("squares: {:?}", squares);
}
