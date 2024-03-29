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
//!     in_circle as f64 / total as f64 * 4.0
//! }
//!
//! let mut rng = thread_rng();
//! let mut in_circle: u64 = 0;
//! let mut observer = Observer::new(Duration::from_secs_f64(0.5));
//! let n: u64 = 10_000_000;
//! for i in 1..n {
//!     let (x, y): (f64, f64) = rng.gen();
//!     if x * x + y * y <= 1.0 {
//!         in_circle += 1;
//!     }
//!     if observer.tick() {
//!         reprint!("pi = {}", pi(i, in_circle));
//!     }
//! }
//! println!("pi = {}", pi(n, in_circle))
//! ```
//!
//! ```
//! use std::time::Duration;
//! use std::io::{stdout, Write};
//! use progress_observer::prelude::*;
//! use rand::prelude::*;
//!
//! // use the observer as an iterator
//!
//! fn pi(total: usize, in_circle: u64) -> f64 {
//!     in_circle as f64 / total as f64 * 4.0
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
//!     if x * x + y * y <= 1.0 {
//!         in_circle += 1;
//!     }
//!     if should_print {
//!         reprint!("pi = {}", pi(i, in_circle));
//!     }
//! }
//! println!("pi = {}", pi(n, in_circle))
//! ```
#![feature(div_duration)]
use std::time::{Duration, Instant};

pub mod prelude {
    pub use super::{reprint, Observer, Options};
}

/// Utility macro for re-printing over the same terminal line.
/// Useful when used in tandem with a progress observer.
///
/// ```
/// use progress_observer::prelude::*;
/// use std::time::Duration;
///
/// // benchmark how many integers you can add per second
///
/// let mut count: u128 = 0;
///
/// for should_print in Observer::new(Duration::from_secs(1)).take(100_000_000) {
///     count += 1;
///     if should_print {
///         reprint!("{count}");
///         count = 0;
///     }
/// }
/// ```
#[macro_export]
macro_rules! reprint {
    ($($tk:tt)*) => {
        {
            print!("\r{}", format!($($tk)*));
            ::std::io::Write::flush(&mut ::std::io::stdout()).unwrap();
        }
    };
}

/// Regular progress update observer.
pub struct Observer {
    frequency_target: Duration,

    checkpoint_size: u64,
    max_checkpoint_size: Option<u64>,
    delay: u64,
    max_scale_factor: f64,
    run_for: Option<Duration>,

    next_checkpoint: u64,
    last_observation: Instant,
    first_observation: Instant,
    ticks: u64,
    finished: bool,
}

/// Optional parameters for creating a new progress observer.
pub struct Options {
    /// Number of ticks before sending the first report.
    ///
    /// The default value of 1 is sometimes quite small, and in combination with the default max_scale_factor of 2,
    /// it can take several dozen iterations before the typical checkpoint size settles to an appropriate value.
    /// These unnecessary extra rapid prints can cause the beginning of your observed timeframe to be crowded with
    /// the expensive syscalls and calculations that might be associated with your operation.
    /// Setting this value to an approximate estimate of the number of iterations you expect to pass within
    /// the time frame specified by your frequency target will prevent this frontloading of printouts.
    pub first_checkpoint: u64,

    /// Specify a maximum number of ticks to wait for in between observations.
    ///
    /// In some instances, such as during particularly chaotic computations, the observer
    /// could erroneously derive an exceedingly large size for the next potential checkpoint. In those situations,
    /// you might want to specify a maximum number of ticks between progress reports, so that
    /// the observer doesn't get stuck waiting indefinitely after a bad next checkpoint estimate.
    pub max_checkpoint_size: Option<u64>,

    /// Delay observations for this many initial ticks.
    ///
    /// Sometimes your computation needs time to "warm up", where the first 1 or 2 iterations may take significantly
    /// longer to process than all subsequent ones. This may throw off the checkpoint estimation. Specify this
    /// argument to ignore the first n ticks processed, only beginning to record progress after they have elapsed.
    pub delay: u64,

    /// Maximum factor that subsequent checkpoints are allowed to increase in size by.
    ///
    /// Intended to prevent sudden large jumps in checkpoint size between reports. The default value of 2 is generally fine for most cases.
    /// Panics if the factor is set less than 1.
    pub max_scale_factor: f64,

    /// Specify a maximum duration to run the observer for.
    ///
    /// After the duration has passed, the observer will return `None` from `Iterator::next`.
    /// Setting this value has no effect if using `Observer::tick` directly.
    pub run_for: Option<Duration>,
}

impl Default for Options {
    fn default() -> Self {
        Self {
            first_checkpoint: 1,
            max_checkpoint_size: None,
            delay: 0,
            max_scale_factor: 2.0,
            run_for: None,
        }
    }
}

impl Observer {
    /// Create an `Observer` with the specified options.
    ///
    /// See the [`Options`] struct for more details on the options that may be specified.
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
    ///     Observer::new_with(Duration::from_secs(1), Options {
    ///         max_checkpoint_size: Some(200_000),
    ///         ..Default::default()
    ///     })
    ///     .take(10_000_000)
    ///     .enumerate()
    /// {
    ///    if is_prime(n as u64) {
    ///        primes += 1;
    ///    }
    ///    if should_print {
    ///        println!("{primes} / {n} = {:.4}", primes as f64 / n as f64);
    ///    }
    /// }
    /// ```
    pub fn new_with(
        frequency_target: Duration,
        Options {
            first_checkpoint: checkpoint_size,
            max_checkpoint_size,
            delay,
            max_scale_factor,
            run_for,
        }: Options,
    ) -> Self {
        if max_scale_factor < 1.0 {
            panic!("max_scale_factor of {max_scale_factor} is less than 1.0");
        }
        Self {
            frequency_target,

            checkpoint_size,
            max_checkpoint_size,
            delay,
            max_scale_factor,
            run_for,

            next_checkpoint: checkpoint_size,
            last_observation: Instant::now(),
            first_observation: Instant::now(),
            ticks: 0,
            finished: false,
        }
    }

    /// Create an `Observer` with a specified starting checkpoint.
    ///
    /// Setting just the starting checkpoint alone is often desirable, so this convenience
    /// method is provided to allow setting it without having to specify a full `Options` struct.
    ///
    /// See the [`Options`] struct for more details on what values to set the starting checkpoint to.
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
    ///     Observer::new_starting_at(Duration::from_secs(1), 300_000)
    ///     .take(10_000_000)
    ///     .enumerate()
    /// {
    ///    if is_prime(n as u64) {
    ///        primes += 1;
    ///    }
    ///    if should_print {
    ///        println!("{primes} / {n} = {:.4}", primes as f64 / n as f64);
    ///    }
    /// }
    /// ```
    pub fn new_starting_at(frequency_target: Duration, first_checkpoint: u64) -> Self {
        Self::new_with(
            frequency_target,
            Options {
                first_checkpoint,
                ..Default::default()
            },
        )
    }

    /// Create a new `Observer` with the specified frequency target and default options.
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
    ///        println!("{primes} / {n} = {:.4}", primes as f64 / n as f64);
    ///    }
    /// }
    /// ```
    pub fn new(frequency_target: Duration) -> Self {
        Self::new_with(frequency_target, Options::default())
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
    ///        println!("{primes} / {n} = {:.4}", primes as f64 / n as f64);
    ///    }
    /// }
    /// ```
    pub fn tick_n(&mut self, mut n: u64) -> bool {
        if self.delay > 0 {
            let adjustment = n.min(self.delay);
            self.delay -= adjustment;
            n -= adjustment;
            if self.delay > 0 {
                return false;
            } else {
                self.last_observation = Instant::now();
                self.first_observation = Instant::now();
            }
        }
        self.ticks += n;
        if self.ticks >= self.next_checkpoint {
            let observation_time = Instant::now();
            if self.run_for.is_some_and(|run_for| {
                observation_time.duration_since(self.first_observation) > run_for
            }) {
                self.finished = true;
            }
            let time_since_observation = observation_time.duration_since(self.last_observation);
            let checkpoint_ratio = time_since_observation.div_duration_f64(self.frequency_target);
            let checkpoint_size = self.checkpoint_size as f64;
            self.checkpoint_size = ((checkpoint_size / checkpoint_ratio) as u64)
                .max(1)
                .min((checkpoint_size * self.max_scale_factor) as u64);
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
    ///        println!("{primes} / {n} = {:.4}", primes as f64 / n as f64);
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
        (!self.finished).then(|| self.tick())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reprint() {
        let mut count: u64 = 0;
        for should_print in Observer::new(Duration::from_secs(1)).take(1_000_000_000) {
            count += 1;
            if should_print {
                reprint!("{count: <20}");
                count = 0;
            }
        }
    }

    #[test]
    fn delay() {
        for (i, should_print) in Observer::new_with(
            Duration::from_secs(1),
            Options {
                max_checkpoint_size: Some(2),
                delay: 5,
                ..Default::default()
            },
        )
        .enumerate()
        .take(10)
        {
            println!("{i}: {should_print}");
        }
    }

    #[test]
    fn run_for() {
        for (i, should_print) in Observer::new_with(
            Duration::from_secs_f32(0.1),
            Options {
                run_for: Some(Duration::from_secs(5)),
                ..Default::default()
            },
        )
        .enumerate()
        {
            if should_print {
                reprint!("{i}");
            }
        }
    }
}
