use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use hdrhistogram::Histogram;
use std::time::{Duration, Instant};

#[derive(Debug, Default)]
pub struct AtomicStats {
    pub requests: AtomicU64,
    pub success: AtomicU64,
    pub errors: AtomicU64,
    pub bytes: AtomicU64,
}

pub struct Statistics {
    stats: Arc<AtomicStats>,
    histogram: Histogram<u64>,
    start_time: Instant,
}

impl Statistics {
    pub fn new() -> Self {
        Statistics {
            stats: Arc::new(AtomicStats::default()),
            histogram: Histogram::<u64>::new(3).expect("Failed to create histogram"),
            start_time: Instant::now(),
        }
    }

    pub fn record_request(&mut self, success: bool, bytes: u64, latency: Duration) {
        self.stats.requests.fetch_add(1, Ordering::Relaxed);
        if success {
            self.stats.success.fetch_add(1, Ordering::Relaxed);
            self.stats.bytes.fetch_add(bytes, Ordering::Relaxed);
            let micros = latency.as_micros() as u64;
            self.histogram.record(micros).unwrap_or_default();
        } else {
            self.stats.errors.fetch_add(1, Ordering::Relaxed);
        }
    }

    pub fn print_stats(&self) {
        let duration = self.start_time.elapsed().as_secs_f64();
        let requests = self.stats.requests.load(Ordering::Relaxed);
        let success = self.stats.success.load(Ordering::Relaxed);
        let errors = self.stats.errors.load(Ordering::Relaxed);
        let bytes = self.stats.bytes.load(Ordering::Relaxed);

        println!("\nStatistics:");
        println!("  Requests/sec: {:.2}", requests as f64 / duration);
        println!("  Transfer/sec: {:.2}MB", bytes as f64 / duration / 1024.0 / 1024.0);
        println!("\nLatency:");
        
        let mean = self.histogram.mean();
        let min = self.histogram.min();
        let max = self.histogram.max();
        let p99 = self.histogram.value_at_quantile(0.99);
        
        println!("  Avg: {:.2}ms", mean / 1000.0);
        println!("  Min: {:.2}ms", min as f64 / 1000.0);
        println!("  Max: {:.2}ms", max as f64 / 1000.0);
        println!("  P99: {:.2}ms", p99 as f64 / 1000.0);
        
        let success_rate = if requests > 0 {
            (success as f64 / requests as f64) * 100.0
        } else {
            0.0
        };
        println!("\nSuccess: {:.2}% ({}/{})", success_rate, success, requests);
        println!("Errors: {:.2}% ({} errors)", (errors as f64 / requests as f64) * 100.0, errors);
    }
} 