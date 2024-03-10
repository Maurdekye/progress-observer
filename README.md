Simple utility for scheduling efficient regular progress updates synchronously on long running, singlethreaded tasks.

Adjusts the interval at which updates are provided automatically based on the length of time taken since the last printout.

As opposed to a naive implementation that checks the system clock at regular, predetermined intervals, this only checks
the system clock exactly once per progress readout. It then observes the time elapsed since the last readout, and uses
that to estimate how many more ticks to wait until it should observe the clock again for the next one. As a result, this
implementation is extremely efficient, while still being able to give regular updates at a desired time interval.

If the execution time of individual steps is too chaotic, then the progress updates may become unpredictable and irregular.
However, the observer's operation is largely resilient to even a moderate amount of irregularity in execution time.

```rs
use std::time::Duration;
use std::io::{stdout, Write};
use progress_observer::prelude::*;
use rand::prelude::*;

// compute pi by generating random points within a square, and checking if they fall within a circle

fn pi(total: u64, in_circle: u64) -> f64 {
    in_circle as f64 / total as f64 * 4.0
}

let mut rng = thread_rng();
let mut in_circle: u64 = 0;
let mut observer = Observer::new(Duration::from_secs_f64(0.5));
let n: u64 = 10_000_000;
for i in 1..n {
    let (x, y): (f64, f64) = rng.gen();
    if x * x + y * y <= 1.0 {
        in_circle += 1;
    }
    if observer.tick() {
        print!("\rpi = {}", pi(i, in_circle));
        stdout().flush().unwrap();
    }
}
println!("pi = {}", pi(n, in_circle))
```

```rs
use std::time::Duration;
use std::io::{stdout, Write};
use progress_observer::prelude::*;
use rand::prelude::*;

// use the observer as an iterator

fn pi(total: usize, in_circle: u64) -> f64 {
    in_circle as f64 / total as f64 * 4.0
}

let mut rng = thread_rng();
let mut in_circle: u64 = 0;
let n = 10_000_000;
for (i, should_print) in
    Observer::new(Duration::from_secs_f64(0.5))
    .take(n)
    .enumerate()
{
    let (x, y): (f64, f64) = rng.gen();
    if x * x + y * y <= 1.0 {
        in_circle += 1;
    }
    if should_print {
        print!("\rpi = {}", pi(i, in_circle));
        stdout().flush().unwrap();
    }
}
println!("pi = {}", pi(n, in_circle))
```