//! Simple utility for scheduling efficient regular progress updates synchronously on long running, singlethreaded tasks.
//!
//! Adjusts the interval at which updates are provided automatically based on the length of time taken since the last printout.
//!
//! As opposed to a naive implementation that checks the system clock at regular, predetermined intervals, this only checks
//! the system clock exactly once per progress readout. It then observes the time elapsed since the last readout, and uses
//! that to estimate how many more ticks to wait until it should observe the clock again for the next one. As a result, this
//! implementation is extremely efficient, while still being able to give regular updates at a desired time interval.
//!
//! If the execution time of individual steps is too chaotic, then the progress updates may become unpredictable and irregular.
//! However, the observer's operation is largely resilient to even a moderate amount of irregularity in execution time.
//!
//! ```
//! use std::time::Duration;
//! use progress_observer::prelude::*;
//! use rand::prelude::*;
//!
//! // compute pi by generating random points within a square, and checking if they fall within a circle
//!
//! fn pi(total: u64, in_circle: u64) -> f64 {
//!     ((total as f64) / (in_circle as f64)) * 4.0
//! }
//!
//! let mut rng = thread_rng();
//! let mut in_circle: u64 = 0;
//! let mut observer = Observer::new(Duration::from_secs_f64(0.5));
//! let n: u64 = 10_000_000;
//! for i in 1..n {
//!     let (x, y): (f64, f64) = rng.gen();
//!     let (x, y) = (x * 2.0 - 1.0, y * 2.0 - 1.0);
//!     if x * x + y * y <= 1.0 {
//!         in_circle += 1;
//!     }
//!     if observer.tick() {
//!         println!("{}", pi(i, in_circle));
//!     }
//! }
//! println!("{}", pi(n, in_circle))
//! ```
//!
//! ```
//! use std::time::Duration;
//! use progress_observer::prelude::*;
//! use rand::prelude::*;
//!
//! // use the observer as an iterator
//!
//! fn pi(total: usize, in_circle: u64) -> f64 {
//!     ((total as f64) / (in_circle as f64)) * 4.0
//! }
//!
//! let mut rng = thread_rng();
//! let mut in_circle: u64 = 0;
//! let n = 10_000_000;
//! for (i, should_print) in
//!     Observer::new(Duration::from_secs_f64(0.5))
//!     .take(n)
//!     .enumerate()
//! {
//!     let (x, y): (f64, f64) = rng.gen();
//!     let (x, y) = (x * 2.0 - 1.0, y * 2.0 - 1.0);
//!     if x * x + y * y <= 1.0 {
//!         in_circle += 1;
//!     }
//!     if should_print {
//!         println!("{}", pi(i, in_circle));
//!     }
//! }
//! println!("{}", pi(n, in_circle))
//! ```
#![feature(div_duration)]
use std::time::{Duration, Instant};

pub mod prelude {
    pub use super::Observer;
}

/// Regular progress update observer.
pub struct Observer {
    frequency_target: Duration,
    checkpoint_size: u64,
    max_checkpoint_size: Option<u64>,
    next_checkpoint: u64,
    last_observation: Instant,
    ticks: u64,
}

impl Observer {
    /// Create an `Observer` with a maximum checkpoint size.
    ///
    /// In some instances, such as during particularly chaotic computations, the observer
    /// could erroneously derive an exceedingly large size for the next potential checkpoint. In those situations,
    /// you might want to specify a maximum number of ticks between progress reports, so that
    /// the observer doesn't get stuck waiting indefinitely after a bad next checkpoint estimate.
    ///
    /// ```
    /// use std::time::Duration;
    /// use std::iter::once;
    /// use progress_observer::prelude::*;
    ///
    /// // compute the ratio of prime numbers between 1 and n
    ///
    /// fn is_prime(n: u64) -> bool {
    ///     once(2)
    ///         .chain((3..=((n as f32).sqrt() as u64)).step_by(2))
    ///         .find(|i| n % i == 0)
    ///         .is_none()
    /// }
    ///
    /// let mut primes = 0;
    /// for (n, should_print) in
    ///     Observer::new_with_max_checkpoint_size(Duration::from_secs(1), 300_000)
    ///     .take(10_000_000)
    ///     .enumerate()
    /// {
    ///    if is_prime(n as u64) {
    ///        primes += 1;
    ///    }
    ///    if should_print {
    ///        println!("{primes} / {n} = {:.4}", (primes as f64) / (n as f64));
    ///    }
    /// }
    /// ```
    pub fn new_with_max_checkpoint_size(
        frequency_target: Duration,
        max_checkpoint_size: u64,
    ) -> Self {
        Self {
            frequency_target,
            checkpoint_size: 1,
            max_checkpoint_size: Some(max_checkpoint_size),
            next_checkpoint: 1,
            last_observation: Instant::now(),
            ticks: 0,
        }
    }

    /// Create an `Observer` with a custom starting checkpoint size.
    ///
    /// The checkpoint size represents the number of ticks until the next progress update is emitted.
    ///
    /// It is adjusted automatically each printout based on the duration of the work performed, and thus it is
    /// typically not necessary to set manually; the default starting checkpoint size of 1 is sufficient for almost any workload,
    /// and the checkpoint size will adjust automatically within 1-3 prints to adapt to the workload you're performing.
    /// Specify only if you both have a strong estimate for how many iterations will pass within the timeframe of your
    /// specified frequency target, *and* you actually care about the frequency of those first couple printouts.
    ///
    /// ```
    /// use std::time::Duration;
    /// use std::iter::once;
    /// use progress_observer::prelude::*;
    ///
    /// // compute the ratio of prime numbers between 1 and n
    ///
    /// fn is_prime(n: u64) -> bool {
    ///     once(2)
    ///         .chain((3..=((n as f32).sqrt() as u64)).step_by(2))
    ///         .find(|i| n % i == 0)
    ///         .is_none()
    /// }
    ///
    /// let mut primes = 0;
    /// for (n, should_print) in
    ///     Observer::new_with_checkpoint_size(Duration::from_secs(1), 500_000)
    ///     .take(10_000_000)
    ///     .enumerate()
    /// {
    ///    if is_prime(n as u64) {
    ///        primes += 1;
    ///    }
    ///    if should_print {
    ///        println!("{primes} / {n} = {:.4}", (primes as f64) / (n as f64));
    ///    }
    /// }
    /// ```
    pub fn new_with_checkpoint_size(frequency_target: Duration, checkpoint_size: u64) -> Self {
        Self {
            frequency_target,
            checkpoint_size,
            max_checkpoint_size: None,
            next_checkpoint: checkpoint_size,
            last_observation: Instant::now(),
            ticks: 0,
        }
    }

    /// Create a new `Observer` with the specified frequency target.
    ///
    /// The observer will attempt to adjust its reports to match the specified target; if you
    /// specify 1 second, it will attempt to display progress updates in 1 second intervals.
    ///
    /// ```
    /// use std::time::Duration;
    /// use std::iter::once;
    /// use progress_observer::prelude::*;
    ///
    /// // compute the ratio of prime numbers between 1 and n
    ///
    /// fn is_prime(n: u64) -> bool {
    ///     once(2)
    ///         .chain((3..=((n as f32).sqrt() as u64)).step_by(2))
    ///         .find(|i| n % i == 0)
    ///         .is_none()
    /// }
    ///
    /// let mut primes = 0;
    /// for (n, should_print) in
    ///     Observer::new(Duration::from_secs(1))
    ///     .take(10_000_000)
    ///     .enumerate()
    /// {
    ///    if is_prime(n as u64) {
    ///        primes += 1;
    ///    }
    ///    if should_print {
    ///        println!("{primes} / {n} = {:.4}", (primes as f64) / (n as f64));
    ///    }
    /// }
    /// ```
    pub fn new(frequency_target: Duration) -> Self {
        Self::new_with_checkpoint_size(frequency_target, 1)
    }

    /// Tick the observer by n iterations at once.
    ///
    /// ```
    /// use std::time::Duration;
    /// use std::iter::once;
    /// use progress_observer::prelude::*;
    ///
    /// // compute the ratio of prime numbers between 1 and n
    ///
    /// fn is_prime(n: u64) -> bool {
    ///     once(2)
    ///         .chain((3..=((n as f32).sqrt() as u64)).step_by(2))
    ///         .find(|i| n % i == 0)
    ///         .is_none()
    /// }
    ///
    /// let mut primes = 0;
    /// let mut observer = Observer::new(Duration::from_secs(1));
    /// for n in 0..10_000_000 {
    ///    if is_prime(n as u64) {
    ///        primes += 1;
    ///    }
    ///    if observer.tick_n(1) {
    ///        println!("{primes} / {n} = {:.4}", (primes as f64) / (n as f64));
    ///    }
    /// }
    /// ```
    pub fn tick_n(&mut self, n: u64) -> bool {
        self.ticks += n;
        if self.ticks >= self.next_checkpoint {
            let observation_time = Instant::now();
            let time_since_observation = observation_time.duration_since(self.last_observation);
            let checkpoint_ratio = time_since_observation.div_duration_f64(self.frequency_target);
            self.checkpoint_size =
                (((self.checkpoint_size as f64) / checkpoint_ratio) as u64).max(1);
            if let Some(max_size) = self.max_checkpoint_size {
                self.checkpoint_size = self.checkpoint_size.min(max_size);
            }
            self.next_checkpoint += self.checkpoint_size;
            self.last_observation = observation_time;
            true
        } else {
            false
        }
    }

    /// Tick the observer by 1 iteration.
    ///
    /// The `tick` method will report a `true` value every time it thinks a progress update
    /// should occur. This is based on the passed frequency_target when the observer is created.
    ///
    /// ```
    /// use std::time::Duration;
    /// use std::iter::once;
    /// use progress_observer::prelude::*;
    ///
    /// // compute the ratio of prime numbers between 1 and n
    ///
    /// fn is_prime(n: u64) -> bool {
    ///     once(2)
    ///         .chain((3..=((n as f32).sqrt() as u64)).step_by(2))
    ///         .find(|i| n % i == 0)
    ///         .is_none()
    /// }
    ///
    /// let mut primes = 0;
    /// let mut observer = Observer::new(Duration::from_secs(1));
    /// for n in 0..10_000_000 {
    ///    if is_prime(n as u64) {
    ///        primes += 1;
    ///    }
    ///    if observer.tick() {
    ///        println!("{primes} / {n} = {:.4}", (primes as f64) / (n as f64));
    ///    }
    /// }
    /// ```
    pub fn tick(&mut self) -> bool {
        self.tick_n(1)
    }
}

impl Iterator for Observer {
    type Item = bool;

    fn next(&mut self) -> Option<Self::Item> {
        Some(self.tick())
    }
}
