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

  private generateHeatmap(): string {
    if (this.latencies.length === 0) return "";

    const maxLatency = Math.max(...this.latencies);
    const bucketCount = 30;
    const bucketSize = Math.ceil(maxLatency / bucketCount);

    // 初始化延迟分桶
    const buckets = new Array(bucketCount).fill(0);
    for (const latency of this.latencies) {
      const bucketIndex = Math.min(
        Math.floor(latency / bucketSize),
        bucketCount - 1,
      );
      buckets[bucketIndex]++;
    }

    // 计算最大高度用于归一化
    const maxCount = Math.max(...buckets);
    const maxBarWidth = 30;

    // 16色梯度（从绿到红）
    const colorPalette = [
      "\x1b[38;5;154m", // 浅绿
      "\x1b[38;5;118m",
      "\x1b[38;5;46m", // 绿色
      "\x1b[38;5;226m", // 黄色
      "\x1b[38;5;208m", // 橙色
      "\x1b[38;5;196m", // 红色
      "\x1b[38;5;124m", // 深红
    ];

    // 生成热力图
    return buckets
      .map((count, index) => {
        const rangeStart = index * bucketSize;
        const rangeEnd = (index + 1) * bucketSize;
        const label = `${rangeStart}~${rangeEnd}ms`.padEnd(12);

        // 计算颜色索引（基于数量密度）
        const intensity = count / maxCount;
        const colorIndex = Math.min(
          Math.floor(intensity * colorPalette.length),
          colorPalette.length - 1,
        );

        // 生成渐变条
        const width = Math.ceil(intensity * maxBarWidth);
        const bar =
          colorPalette[colorIndex] + ("█".repeat(width) || "▏") + "\x1b[0m"; // 重置颜色

        // 统计信息
        const percent = ((count / this.latencies.length) * 100).toFixed(1);
        return `${label} ${bar} ${count.toString().padEnd(5)} (${percent}%)`;
      })
      .filter((_, index) => {
        // 只显示：前5个桶 + 有数据的桶 + 最后5个桶
        return index < 5 || buckets[index] > 0 || index >= bucketCount - 5;
      })
      .join("\n");
  }

  private generateFlameGraph(): string {
    if (this.latencies.length === 0) return "";

    // 确保最小延迟不为零
    const nonZeroLatencies = this.latencies.filter((n) => n > 0);
    if (nonZeroLatencies.length === 0) return "";

    const minLatency = Math.min(...nonZeroLatencies);
    const maxLatency = Math.max(...this.latencies);

    // 计算对数范围
    const logMin = Math.log10(minLatency);
    const logMax = Math.log10(maxLatency);
    const logRange = logMax - logMin;

    // 预计算分桶边界
    const bucketCount = 30;
    const timeMarkers = Array.from({ length: bucketCount + 1 }, (_, i) => {
      return Math.pow(10, logMin + (i / bucketCount) * logRange);
    });

    // 初始化分桶
    const buckets = new Array(bucketCount).fill(0);
    for (const latency of this.latencies) {
      const logVal = Math.log10(latency);
      const ratio = (logVal - logMin) / logRange;
      const bucketIndex = Math.min(
        Math.floor(ratio * bucketCount),
        bucketCount - 1,
      );
      buckets[bucketIndex]++;
    }

    // 颜色配置（与热力图一致）
    const colorPalette = [
      "\x1b[38;5;154m", // 浅绿
      "\x1b[38;5;118m",
      "\x1b[38;5;46m", // 绿色
      "\x1b[38;5;226m", // 黄色
      "\x1b[38;5;208m", // 橙色
      "\x1b[38;5;196m", // 红色
      "\x1b[38;5;124m", // 深红
    ];

    // 计算最大对数计数
    const maxLog = Math.log10(Math.max(...buckets) + 1);

    return buckets
      .map((count, index) => {
        if (count === 0) return null;

        // 获取精确范围
        const min = timeMarkers[index];
        const max = timeMarkers[index + 1];

        // 格式化标签
        const format = (n: number) =>
          n >= 10 ? n.toFixed(0) : n.toFixed(n < 1 ? 1 : 1);
        const label = `${format(min)}~${format(max)}ms`.padEnd(12);

        // 基于中值延迟选择颜色
        const midValue = (min + max) / 2;
        const colorRatio = midValue / maxLatency;
        const colorIndex = Math.min(
          Math.floor(colorRatio * colorPalette.length),
          colorPalette.length - 1,
        );

        // 对数宽度计算
        const logCount = Math.log10(count + 1);
        const width = Math.ceil((logCount / maxLog) * 30);

        // 生成条形
        const bar = colorPalette[colorIndex] + "█".repeat(width) + "\x1b[0m";

        // 统计信息
        const percent = ((count / this.latencies.length) * 100).toFixed(1);
        return `${label} ${bar} ${count.toString().padEnd(5)} (${percent}%)`;
      })
      .filter((line) => line !== null) // 过滤空桶
      .join("\n");
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

      console.log("\n🔥 Latency Distribution Heatmap:");
      console.log(this.generateHeatmap());

      console.log("\n🌋 Latency Flame Graph (Log Scale):");
      console.log(this.generateFlameGraph());
    }
  }
}

// 命令行参数解析
function parseArgs(): BenchmarkConfig {
  const args = Bun.argv; // bun only!
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
