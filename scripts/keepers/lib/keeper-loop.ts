/**
 * Generic keeper loop with retry, backoff, and graceful shutdown.
 *
 * Usage:
 *   await runKeeperLoop(
 *     { name: "compound", intervalMs: 300_000, maxRetries: 3, retryBaseMs: 2000 },
 *     async () => { ... },
 *   );
 */

import { createLogger } from "./logger.js";

export interface KeeperConfig {
  name: string;
  intervalMs: number;
  maxRetries: number;
  retryBaseMs: number;
}

function sleep(ms: number): Promise<void> {
  return new Promise((r) => setTimeout(r, ms));
}

async function interruptibleSleep(
  ms: number,
  shouldStop: () => boolean,
): Promise<void> {
  const step = 1000;
  let elapsed = 0;
  while (elapsed < ms && !shouldStop()) {
    await sleep(Math.min(step, ms - elapsed));
    elapsed += step;
  }
}

export async function runKeeperLoop(
  config: KeeperConfig,
  tick: () => Promise<void>,
): Promise<void> {
  const log = createLogger(config.name);
  let running = true;
  let retries = 0;

  const shutdown = (signal: string) => {
    log.info("Shutdown requested", { signal });
    running = false;
  };
  process.on("SIGINT", () => shutdown("SIGINT"));
  process.on("SIGTERM", () => shutdown("SIGTERM"));

  log.info("Starting keeper loop", { intervalMs: config.intervalMs });

  // Run first tick immediately
  while (running) {
    try {
      await tick();
      retries = 0;
    } catch (err: any) {
      retries++;
      const backoff = Math.min(
        config.retryBaseMs * 2 ** (retries - 1),
        60_000,
      );
      log.error("Tick failed", {
        error: err.message,
        retry: retries,
        backoffMs: backoff,
        logs: err.logs?.slice(-5),
      });
      if (retries >= config.maxRetries) {
        log.error("Max retries reached, waiting full interval");
        retries = 0;
      } else {
        await sleep(backoff);
        continue; // retry immediately
      }
    }

    await interruptibleSleep(config.intervalMs, () => !running);
  }

  log.info("Keeper loop exited cleanly");
}
