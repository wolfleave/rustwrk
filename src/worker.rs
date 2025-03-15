use anyhow::{Result, Error};
use hyper::Uri;
use hyper_util::client::legacy::Client as HyperClient;
use hyper_util::rt::TokioExecutor;
use hyper_tls::HttpsConnector;
use hyper_util::client::legacy::connect::HttpConnector;
use std::time::{Duration, Instant};
use tokio::time;
use http_body_util::{Empty, BodyExt};
use hyper::body::Bytes;
use crate::stats::Statistics;

type Client = HyperClient<HttpsConnector<HttpConnector>, Empty<Bytes>>;
type StatsResult = Result<(u64, u64, u64, u64, Duration)>;

pub struct Worker {
    client: Client,
    stats: Statistics,
    connections: usize,
}

impl Worker {
    pub fn new(connections: usize) -> Self {
        let mut http = HttpConnector::new();
        http.enforce_http(false);
        let https = HttpsConnector::new_with_connector(http);
        let client = HyperClient::builder(TokioExecutor::new())
            .pool_idle_timeout(Duration::from_secs(30))
            .build(https);

        Worker {
            client,
            stats: Statistics::new(),
            connections,
        }
    }

    pub async fn run(&mut self, url: String, duration: Duration, timeout: Duration) -> Result<()> {
        let uri = url.parse::<Uri>()?;
        let end_time = Instant::now() + duration;

        let mut handles = Vec::with_capacity(self.connections);

        for _ in 0..self.connections {
            let client = self.client.clone();
            let uri = uri.clone();

            let handle: tokio::task::JoinHandle<StatsResult> = tokio::spawn(async move {
                let mut requests = 0u64;
                let mut successes = 0u64;
                let mut total_bytes = 0u64;
                let mut total_latency = Duration::default();
                let mut errors = 0u64;
                
                while Instant::now() < end_time {
                    let start = Instant::now();
                    let req = hyper::Request::builder()
                        .method(hyper::Method::GET)
                        .uri(uri.clone())
                        .body(Empty::<Bytes>::new())
                        .unwrap();

                    requests += 1;
                    match time::timeout(timeout, client.request(req)).await {
                        Ok(Ok(resp)) => {
                            let status = resp.status();
                            let body = resp.into_body();
                            let bytes = match body.collect().await {
                                Ok(collected) => collected.to_bytes().len(),
                                Err(_) => 0,
                            };
                            let latency = start.elapsed();
                            
                            if status.is_success() {
                                successes += 1;
                                total_bytes += bytes as u64;
                                total_latency += latency;
                            } else {
                                errors += 1;
                                tracing::error!("HTTP error: {}", status);
                            }
                        }
                        Ok(Err(e)) => {
                            let latency = start.elapsed();
                            errors += 1;
                            tracing::error!("Request error: {}", e);
                            total_latency += latency;
                        }
                        Err(_) => {
                            let latency = start.elapsed();
                            errors += 1;
                            tracing::error!("Request timeout");
                            total_latency += latency;
                        }
                    }
                }
                Ok((requests, successes, errors, total_bytes, total_latency))
            });
            handles.push(handle);
        }

        let mut total_requests = 0;
        let mut total_successes = 0;
        let mut total_errors = 0;
        let mut total_bytes = 0;
        let mut total_latency = Duration::default();

        for handle in handles {
            if let Ok(Ok((requests, successes, errors, bytes, latency))) = handle.await {
                total_requests += requests;
                total_successes += successes;
                total_errors += errors;
                total_bytes += bytes;
                total_latency += latency;
                
                // 记录每个请求的延迟
                if requests > 0 {
                    let avg_latency = Duration::from_nanos((latency.as_nanos() / requests as u128) as u64);
                    self.stats.record_request(successes > 0, bytes / requests, avg_latency);
                }
            }
        }

        println!("\nSummary:");
        println!("Total Requests: {}", total_requests);
        println!("Successful Requests: {}", total_successes);
        println!("Failed Requests: {}", total_errors);
        if total_requests > 0 {
            println!("Success Rate: {:.2}%", (total_successes as f64 / total_requests as f64) * 100.0);
            println!("Average Latency: {:.2}ms", total_latency.as_secs_f64() * 1000.0 / total_requests as f64);
            println!("Total Bytes: {:.2}MB", total_bytes as f64 / 1024.0 / 1024.0);
        }
        
        self.stats.print_stats();
        Ok(())
    }
} 