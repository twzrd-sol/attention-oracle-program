/**
 * Structured JSON logger for keeper processes.
 *
 * Every log line is a single JSON object for easy ingestion by
 * journald / CloudWatch / Datadog / jq pipelines.
 */

export type LogLevel = "debug" | "info" | "warn" | "error";

export interface LogEntry {
  ts: string;
  level: LogLevel;
  keeper: string;
  msg: string;
  [key: string]: unknown;
}

function emit(
  level: LogLevel,
  keeper: string,
  msg: string,
  data?: Record<string, unknown>,
) {
  const entry: LogEntry = {
    ts: new Date().toISOString(),
    level,
    keeper,
    msg,
    ...data,
  };
  const stream = level === "error" ? process.stderr : process.stdout;
  stream.write(JSON.stringify(entry) + "\n");
}

export interface Logger {
  debug(msg: string, data?: Record<string, unknown>): void;
  info(msg: string, data?: Record<string, unknown>): void;
  warn(msg: string, data?: Record<string, unknown>): void;
  error(msg: string, data?: Record<string, unknown>): void;
}

export function createLogger(keeper: string): Logger {
  return {
    debug: (msg, data) => emit("debug", keeper, msg, data),
    info: (msg, data) => emit("info", keeper, msg, data),
    warn: (msg, data) => emit("warn", keeper, msg, data),
    error: (msg, data) => emit("error", keeper, msg, data),
  };
}
