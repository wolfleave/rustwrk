mod stats;
mod worker;

use std::time::Duration;
use clap::Parser;
use anyhow::Result;
use url::Url;
use worker::Worker;

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// Number of threads to use
    #[arg(short = 't', default_value_t = num_cpus::get())]
    threads: usize,

    /// Number of connections to keep open
    #[arg(short = 'c', default_value_t = 100)]
    connections: usize,

    /// Duration of the test in seconds
    #[arg(short = 'd', default_value_t = 10)]
    duration: u64,

    /// Timeout for each request in seconds
    #[arg(short = 'T', default_value_t = 5)]
    timeout: u64,

    /// Target URL
    #[arg(required = true)]
    url: String,
}

#[derive(Debug, Default)]
struct Stats {
    requests: u64,
    success: u64,
    errors: u64,
    bytes: u64,
    latency_min: u64,
    latency_max: u64,
    latency_sum: u64,
}

#[tokio::main]
async fn main() -> Result<()> {
    // 初始化日志
    tracing_subscriber::fmt::init();

    // 解析命令行参数
    let args = Args::parse();

    // 验证URL
    let _url = Url::parse(&args.url)?;

    println!("Running {}s test @ {}", args.duration, args.url);
    println!("  {} threads and {} connections", args.threads, args.connections);
    println!();

    let connections_per_thread = args.connections / args.threads;
    let mut handles = Vec::with_capacity(args.threads);

    // 启动工作线程
    for _ in 0..args.threads {
        let url = args.url.clone();
        let duration = Duration::from_secs(args.duration);
        let timeout = Duration::from_secs(args.timeout);
        
        let handle = tokio::spawn(async move {
            let mut worker = Worker::new(connections_per_thread);
            worker.run(url, duration, timeout).await
        });
        
        handles.push(handle);
    }

    // 等待所有线程完成
    for handle in handles {
        handle.await??;
    }

    Ok(())
} 