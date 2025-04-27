// 使用 Bun 运行的基准测试工具
import { exit } from "node:process";

// ========================
// 类型定义
// ========================
interface BenchmarkConfig {
  url: string;
  concurrency?: number;
  durationSeconds?: number;
}

interface LatencyStats {
  avg: number;
  p95: number;
  p99: number;
  max: number;
}

// ========================
// 常量定义
// ========================
const DEFAULT_CONCURRENCY = 10;
const DEFAULT_DURATION = 60;
const REPORT_INTERVAL_MS = 1000;
const STATS_REPORT_INTERVAL_MS = 10000; // 每10秒输出统计信息
const HISTOGRAM_BUCKETS = 30;
const COLOR_PALETTE = [
  "\x1b[38;5;154m", // 浅绿色
  "\x1b[38;5;118m",
  "\x1b[38;5;46m", // 绿色
  "\x1b[38;5;226m", // 黄色
  "\x1b[38;5;208m", // 橙色
  "\x1b[38;5;196m", // 红色
  "\x1b[38;5;124m", // 深红色
];

// ========================
// 主要基准测试类
// ========================
class Benchmarker {
  private totalRequests = 0;
  private successful = 0;
  private failed = 0;
  private isRunning = true;
  private lastReport = 0;
  private latencies: number[] = [];

  constructor(private config: BenchmarkConfig) {
    this.config.concurrency ??= DEFAULT_CONCURRENCY;
    this.config.durationSeconds ??= DEFAULT_DURATION;
  }

  async run() {
    console.log(
      `🚀 开始基准测试，使用 ${this.config.concurrency} 个工作线程...`,
    );

    const reportInterval = setInterval(() => this.report(), REPORT_INTERVAL_MS);
    const statsReportInterval = setInterval(
      () => this.reportStats(),
      STATS_REPORT_INTERVAL_MS,
    );
    const workers = Array.from({ length: this.config.concurrency! }, () =>
      this.worker(),
    );

    setTimeout(() => {
      this.isRunning = false;
      clearInterval(reportInterval);
      clearInterval(statsReportInterval);
      this.finalReport();
    }, this.config.durationSeconds! * 1000);

    await Promise.all(workers);
  }

  // ========================
  // 私有方法
  // ========================
  private async worker() {
    while (this.isRunning) {
      try {
        const start = Date.now();
        const response = await fetch(this.config.url);

        response.ok ? this.successful++ : this.failed++;
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

    const reportLines = [
      `🕒 ${new Date().toLocaleTimeString()}`,
      `⚡ RPS: ${rps}/s`,
      `✅ 成功: ${this.successful}`,
      `❌ 失败: ${this.failed}`,
    ];

    if (currentStats) {
      reportLines.push(`⏳ 平均: ${currentStats.avg.toFixed(1)}ms`);
    }

    console.log(reportLines.join(" | "));
  }

  private reportStats() {
    const stats = this.calculateLatencyStats();
    if (!stats || this.totalRequests === 0) return;

    console.log("\n=== 当前统计信息 (每10秒更新) ===");
    console.log(`🏁 总请求数: ${this.totalRequests}`);
    console.log(
      `🟢 成功: ${this.successful} (${successRate(this.successful, this.totalRequests)}%)`,
    );
    console.log(
      `🔴 失败: ${this.failed} (${successRate(this.failed, this.totalRequests)}%)`,
    );
    console.log(`⏱️  延迟统计:`);
    console.log(`📊 平均值: ${stats.avg.toFixed(2)}ms`);
    console.log(`📈 P95: ${stats.p95}ms`);
    console.log(`📉 P99: ${stats.p99}ms`);
    console.log(`🚀 最大值: ${stats.max}ms`);

    // 生成简化版延迟分布图
    console.log("\n🔥 延迟分布:");
    console.log(generateHeatmap(this.latencies, COLOR_PALETTE));
    console.log(); // 空行分隔
  }

  private finalReport() {
    const stats = this.calculateLatencyStats();

    console.log("\n=== 基准测试完成 ===");
    console.log(`🏁 总请求数: ${this.totalRequests}`);
    console.log(
      `🟢 成功: ${this.successful} (${successRate(this.successful, this.totalRequests)}%)`,
    );
    console.log(
      `🔴 失败: ${this.failed} (${successRate(this.failed, this.totalRequests)}%)`,
    );
    console.log(`⏱️  持续时间: ${this.config.durationSeconds}秒`);

    if (stats) {
      console.log("\n⏳ 延迟统计:");
      console.log(`📊 平均值: ${stats.avg.toFixed(2)}ms`);
      console.log(`📈 P95: ${stats.p95}ms`);
      console.log(`📉 P99: ${stats.p99}ms`);
      console.log(`🚀 最大值: ${stats.max}ms`);

      console.log("\n🔥 延迟分布热力图:");
      console.log(generateHeatmap(this.latencies, COLOR_PALETTE));

      console.log("\n🌋 延迟火焰图 (对数刻度):");
      console.log(generateFlameGraph(this.latencies, COLOR_PALETTE));
    }
  }

  private calculateLatencyStats(): LatencyStats | null {
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
}

// ========================
// 可视化工具函数
// ========================
function generateHeatmap(latencies: number[], colors: string[]): string {
  if (latencies.length === 0) return "";

  const maxLatency = Math.max(...latencies);
  const bucketSize = Math.ceil(maxLatency / HISTOGRAM_BUCKETS);
  const buckets = new Array(HISTOGRAM_BUCKETS).fill(0);

  latencies.forEach((latency) => {
    const bucketIndex = Math.min(
      Math.floor(latency / bucketSize),
      HISTOGRAM_BUCKETS - 1,
    );
    buckets[bucketIndex]++;
  });

  // 计算最大计数和最大宽度
  const maxCount = Math.max(...buckets);
  const maxBarWidth = 18; // 固定最大条形宽度

  // 找出最长的数字宽度
  const maxDigits = Math.max(
    ...buckets.filter((c) => c > 0).map((c) => c.toString().length),
  );

  return buckets
    .map((count, index) => {
      if (count === 0) return null;

      const rangeStart = index * bucketSize;
      const rangeEnd = (index + 1) * bucketSize;
      const label =
        `${formatNumber(rangeStart)}~${formatNumber(rangeEnd)}ms`.padEnd(12);

      // 线性宽度计算
      const width = Math.max(1, Math.floor((count / maxCount) * maxBarWidth));

      // 计算颜色（基于线性比例）
      const colorRatio = count / maxCount;
      const colorIndex = Math.min(
        Math.floor(colorRatio * colors.length),
        colors.length - 1,
      );

      // 计算百分比
      const percent = ((count / latencies.length) * 100).toFixed(1);

      // 右对齐计数
      const countStr = count.toString();
      // 注意：这里条形图直接连接数字，没有空格
      const bar = colors[colorIndex] + "█".repeat(width) + "\x1b[0m";

      return `${label} ${bar}${countStr}  (${percent}%)`;
    })
    .filter((line) => line !== null)
    .join("\n");
}

function generateFlameGraph(latencies: number[], colors: string[]): string {
  const nonZero = latencies.filter((n) => n > 0);
  if (nonZero.length === 0) return "";

  const min = Math.min(...nonZero);
  const max = Math.max(...nonZero);
  const logMin = Math.log10(min);
  const logMax = Math.log10(max);
  const logRange = logMax - logMin;

  // 对数时间间隔计算
  const timeMarkers = Array.from({ length: HISTOGRAM_BUCKETS + 1 }, (_, i) =>
    Math.pow(10, logMin + (i / HISTOGRAM_BUCKETS) * logRange),
  );

  // 构建分桶
  const buckets = new Array(HISTOGRAM_BUCKETS).fill(0);
  nonZero.forEach((latency) => {
    const ratio = (Math.log10(latency) - logMin) / logRange;
    const bucketIndex = Math.min(
      Math.floor(ratio * HISTOGRAM_BUCKETS),
      HISTOGRAM_BUCKETS - 1,
    );
    buckets[bucketIndex]++;
  });

  // 计算最大计数和对数最大计数
  const maxCount = Math.max(...buckets);
  const maxLogCount = Math.log10(maxCount + 1);
  const maxBarWidth = 18; // 固定最大条形宽度

  // 找出最长的数字宽度
  const maxDigits = Math.max(
    ...buckets.filter((c) => c > 0).map((c) => c.toString().length),
  );

  return buckets
    .map((count, index) => {
      if (count === 0) return null;

      // 获取对数时间范围边界
      const rangeStart = timeMarkers[index];
      const rangeEnd = timeMarkers[index + 1];
      const label =
        `${formatNumber(rangeStart)}~${formatNumber(rangeEnd)}ms`.padEnd(12);

      // 对数宽度计算
      const logWidth = Math.ceil(
        (Math.log10(count + 1) / maxLogCount) * maxBarWidth,
      );
      const width = Math.max(1, logWidth);

      // 计算颜色（基于时间范围）
      const midValue = (rangeStart + rangeEnd) / 2;
      const colorRatio = (Math.log10(midValue) - logMin) / logRange;
      const colorIndex = Math.min(
        Math.floor(colorRatio * colors.length),
        colors.length - 1,
      );

      // 计算百分比
      const percent = ((count / latencies.length) * 100).toFixed(1);

      // 右对齐计数，条形图直接连接数字
      const countStr = count.toString();
      const bar = colors[colorIndex] + "█".repeat(width) + "\x1b[0m";

      return `${label} ${bar}${countStr}  (${percent}%)`;
    })
    .filter((line) => line !== null)
    .join("\n");
}

// ========================
// 辅助函数
// ========================
function formatBuckets(
  buckets: number[],
  bucketSize: number,
  colors: string[],
  total: number,
  timeMarkers?: number[],
): string {
  const maxCount = Math.max(...buckets);
  const maxBarWidth = 30;

  return buckets
    .map((count, index) => {
      if (count === 0) return null;

      const [rangeStart, rangeEnd] = timeMarkers
        ? [timeMarkers[index], timeMarkers[index + 1]]
        : [index * bucketSize, (index + 1) * bucketSize];

      const label =
        `${formatNumber(rangeStart)}~${formatNumber(rangeEnd)}ms`.padEnd(12);
      const percent = ((count / total) * 100).toFixed(1);
      const intensity = count / maxCount;
      const colorIndex = Math.min(
        Math.floor(intensity * colors.length),
        colors.length - 1,
      );
      const width = Math.ceil(intensity * maxBarWidth);

      return `${label} ${colors[colorIndex]}${"█".repeat(width)}\x1b[0m ${count.toString().padEnd(5)} (${percent}%)`;
    })
    .filter((line) => line !== null)
    .join("\n");
}

function formatNumber(n: number): string {
  return n >= 10 ? n.toFixed(0) : n.toFixed(n < 1 ? 1 : 1);
}

function successRate(count: number, total: number): string {
  return ((count / total) * 100).toFixed(1);
}

// ========================
// 命令行实现
// ========================
function parseArgs(): BenchmarkConfig {
  const args = Bun.argv;
  const config: BenchmarkConfig = { url: "" };

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
HTTP 压测工具
用法: bun run request.ts [选项]

选项:
  -u, --url <url>          目标URL (必需)
  -c, --concurrency <num>  并发工作线程数 (默认: ${DEFAULT_CONCURRENCY})
  -d, --duration <sec>     测试持续时间（秒）(默认: ${DEFAULT_DURATION})
  -h, --help               显示帮助信息

示例:
  bun run request.ts --url https://api.example.com --concurrency 20 --duration 120
`);
}

// ========================
// 主程序执行
// ========================
try {
  const config = parseArgs();
  const benchmark = new Benchmarker(config);
  benchmark.run();
} catch (error) {
  console.error(
    "🚨 基准测试失败:",
    error instanceof Error ? error.message : error,
  );
  exit(1);
}
