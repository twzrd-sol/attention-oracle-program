import http from 'http';
import pino from 'pino';

const logger = pino({ name: 'metrics' });

const counters = {
  events_emitted: 0,
  gateway_success: 0,
  gateway_failure: 0,
  protocol_updates: 0,
};

export const metrics = {
  inc: (key: keyof typeof counters) => {
    counters[key]++;
  },
  server: http.createServer((req, res) => {
    if (req.url === '/metrics') {
      res.writeHead(200, { 'Content-Type': 'text/plain' });
      const lines = Object.entries(counters).map(
        ([k, v]) => `stream_listener_${k}_total ${v}`,
      );
      res.end(lines.join('\n'));
    } else {
      res.writeHead(404);
      res.end();
    }
  }),
};

export const startMetricsServer = (port = 9091) => {
  metrics.server.listen(port, () => {
    logger.info({ port }, 'Metrics server listening');
  });
};

