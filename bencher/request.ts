// 请使用 bun 运行
import { exit } from "node:process";

interface BenchmarkConfig {
  url: string;
  concurrency?: number;
  durationSeconds?: number;
}

class Benchmarker {
  private totalRequests = 0;
  private successful = 0;
  private failed = 0;
  private isRunning = true;
  private lastReport = 0;
  private latencies: number[] = [];

  constructor(private config: BenchmarkConfig) {
    this.config.concurrency ??= 10;
    this.config.durationSeconds ??= 60;
  }

  async run() {
    console.log(
      `🚀 Starting benchmark with ${this.config.concurrency} workers...`,
    );

    // 启动统计报告
    const reportInterval = setInterval(() => this.report(), 1000);

    // 创建工作线程
    const workers = Array.from({ length: this.config.concurrency! }, () =>
      this.worker(),
    );

    // 设置超时停止
    setTimeout(() => {
      this.isRunning = false;
      clearInterval(reportInterval);
      this.finalReport();
    }, this.config.durationSeconds! * 1000);

    await Promise.all(workers);
  }

  private async worker() {
    while (this.isRunning) {
      try {
        const start = Date.now();
        const response = await fetch(this.config.url);

        if (response.ok) {
          this.successful++;
        } else {
          this.failed++;
        }

        this.totalRequests++;
        this.trackLatency(Date.now() - start);
      } catch (error) {
        this.failed++;
        this.totalRequests++;
      }
    }
  }

  private trackLatency(ms: number) {
    this.latencies.push(ms);
  }

  private report() {
    const rps = this.totalRequests - this.lastReport;
    const currentStats = this.calculateLatencyStats();
    this.lastReport = this.totalRequests;

    const lines = [
      `🕒 ${new Date().toLocaleTimeString()}`,
      `⚡ RPS: ${rps}/s`,
      `✅ Success: ${this.successful}`,
      `❌ Failed: ${this.failed}`,
    ];

    if (currentStats) {
      lines.push(`⏳ Avg: ${currentStats.avg.toFixed(1)}ms`);
    }

    console.log(lines.join(" | "));
  }

  private calculateLatencyStats() {
    if (this.latencies.length === 0) return null;

    const sorted = [...this.latencies].sort((a, b) => a - b);
    const total = sorted.reduce((a, b) => a + b, 0);

    return {
      avg: total / sorted.length,
      p95: sorted[Math.floor(sorted.length * 0.95)],
      p99: sorted[Math.floor(sorted.length * 0.99)],
      max: sorted[sorted.length - 1],
    };
  }

  private finalReport() {
    const stats = this.calculateLatencyStats();

    console.log("\n=== Benchmark Complete ===");
    console.log(`🏁 Total Requests: ${this.totalRequests}`);
    console.log(
      `🟢 Successful: ${this.successful} (${((this.successful / this.totalRequests) * 100).toFixed(1)}%)`,
    );
    console.log(
      `🔴 Failed: ${this.failed} (${((this.failed / this.totalRequests) * 100).toFixed(1)}%)`,
    );
    console.log(`⏱️  Duration: ${this.config.durationSeconds}s`);

    if (stats) {
      console.log("\n⏳ Latency Statistics:");
      console.log(`📊 Average: ${stats.avg.toFixed(2)}ms`);
      console.log(`📈 P95: ${stats.p95}ms`);
      console.log(`📉 P99: ${stats.p99}ms`);
      console.log(`🚀 Max: ${stats.max}ms`);
    }
  }
}

// 命令行参数解析
function parseArgs(): BenchmarkConfig {
  const args = Bun.argv;
  const config: BenchmarkConfig = {
    url: "",
    concurrency: 10,
    durationSeconds: 60,
  };

  for (let i = 2; i < args.length; i++) {
    switch (args[i]) {
      case "--url":
      case "-u":
        config.url = args[++i];
        break;
      case "--concurrency":
      case "-c":
        config.concurrency = parseInt(args[++i], 10);
        break;
      case "--duration":
      case "-d":
        config.durationSeconds = parseInt(args[++i], 10);
        break;
      case "--help":
      case "-h":
        showHelp();
        exit(0);
    }
  }

  if (!config.url) {
    console.error("❌ 必须指定目标URL（使用 --url 参数）");
    showHelp();
    exit(1);
  }

  return config;
}

function showHelp() {
  console.log(`
压测工具使用说明：
Usage: bun run request.ts [options]

Options:
  -u, --url <url>          目标URL (必需)
  -c, --concurrency <num>  并发数 (默认: 10)
  -d, --duration <sec>     持续时间（秒）(默认: 60)
  -h, --help               显示帮助信息
`);
}

// 主程序
const config = parseArgs();
const benchmark = new Benchmarker(config);
benchmark.run();
