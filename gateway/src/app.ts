/**
 * Gateway Application Setup
 *
 * Main Expreå\
 * 
 * \\\ss app with:
 * - Portal v3 static serving
 * - API routes (/api/verification-status, /api/claim-cls)
 * - CORS, error handling, logging
 */

import express, { Express, Request, Response, NextFunction } from 'express';
import path from 'path';
import cors from 'cors';
import { fileURLToPath } from 'url';
import { dirname } from 'path';
import { setupApiRoutes } from './api/routes.js';
import { metricsHandler, updateEpochMetrics, updateViewerMetrics } from './metrics.js';
import { db } from './db.js';
import bindingsRouter from './routes/bindings.js';

// ES module polyfill for __dirname
const __filename = fileURLToPath(import.meta.url);
const __dirname = dirname(__filename);

/**
 * Create and configure Express app
 */
export function createApp(): Express {
  const app = express();

  // ===== Middleware =====

  // CORS (adjust origins as needed)
  app.use(
    cors({
      origin: (origin, callback) => {
        const allowedOrigins = process.env.ALLOWED_ORIGINS?.split(',') || [];
        const defaultAllowed = [
          'http://localhost:3000',
          'https://twzrd.xyz'
        ];

        // Allow requests with no origin (e.g., mobile apps, curl)
        if (!origin) {
          return callback(null, true);
        }

        // Check if origin is explicitly allowed
        if (allowedOrigins.includes(origin) || defaultAllowed.includes(origin)) {
          return callback(null, true);
        }

        // Allow all Cloudflare Pages preview URLs
        if (origin.endsWith('.attention-oracle-portal.pages.dev')) {
          return callback(null, true);
        }

        callback(new Error('Not allowed by CORS'));
      },
      credentials: true,
      methods: ['GET', 'POST', 'PUT', 'DELETE', 'OPTIONS'],
      allowedHeaders: ['Content-Type', 'Authorization'],
    })
  );

  // Wildcard preflight fallback (catches any missed sub-router OPTIONS)
  app.options('/api/*', (req, res) => {
    const origin = req.headers.origin as string | undefined;

    const allowed = !origin ||
      origin === 'https://twzrd.xyz' ||
      origin.startsWith('http://localhost:') ||
      origin.endsWith('.attention-oracle-portal.pages.dev');

    if (allowed) {
      res.setHeader('Access-Control-Allow-Origin', origin || 'https://twzrd.xyz');
      res.setHeader('Access-Control-Allow-Credentials', 'true');
      res.setHeader('Access-Control-Allow-Methods', 'GET, POST, OPTIONS');
      res.setHeader('Access-Control-Allow-Headers', 'Content-Type, Authorization');
    }

    res.status(204).end();
  });

  // JSON parser
  app.use(express.json());
  app.use(express.urlencoded({ extended: true }));

  // Request logging (optional)
  app.use((req: Request, _res: Response, next: NextFunction) => {
    console.log(`[${new Date().toISOString()}] ${req.method} ${req.path}`);
    next();
  });

  // ===== Health Check Endpoint =====

  app.get('/health', async (_req: Request, res: Response) => {
    try {
      // Optional: lightweight DB check
      // await db.oneOrNone('SELECT 1');

      res.status(200).json({
        status: 'ok',
        uptimeSeconds: Math.floor(process.uptime()),
        timestamp: new Date().toISOString(),
        version: process.env.GATEWAY_VERSION || '1.0.0',
      });
    } catch (error) {
      console.error('[Health] Error during health check:', error);
      res.status(500).json({
        status: 'error',
        error: 'health_check_failed',
        timestamp: new Date().toISOString(),
      });
    }
  });

  // ===== Metrics Endpoint =====

  // Prometheus metrics (must be BEFORE static serving to avoid conflict)
  app.get('/metrics', metricsHandler);

  // ===== Static Portal v3 Assets =====

  const portalPath = path.join(__dirname, '..', '..', 'portal-v3', 'dist');
  console.log(`[App] Serving portal-v3 from: ${portalPath}`);

  // Serve static assets (JS, CSS, etc.)
  app.use(express.static(portalPath));

  // ===== API Routes =====

  // Explicit preflight handler for bind-wallet (fixes 405 when cors() alone fails to catch)
  app.options('/api/bindings/bind-wallet', (req: Request, res: Response) => {
    const origin = req.headers.origin as string | undefined;

    const allowed = !origin ||
      origin === 'https://twzrd.xyz' ||
      origin.startsWith('http://localhost:') ||
      origin.endsWith('.attention-oracle-portal.pages.dev');

    if (allowed) {
      res.setHeader('Access-Control-Allow-Origin', origin || 'https://twzrd.xyz');
      res.setHeader('Access-Control-Allow-Credentials', 'true');
      res.setHeader('Access-Control-Allow-Methods', 'POST, OPTIONS');
      res.setHeader('Access-Control-Allow-Headers', 'Content-Type, Authorization');
    }

    res.status(204).end();
  });

  app.use('/api/bindings', bindingsRouter);

  const apiRouter = setupApiRoutes();
  app.use('/api', apiRouter);

  // ===== Error Handling =====

  // 404 handler for unknown API routes
  app.use('/api', (_req: Request, res: Response) => {
    res.status(404).json({
      error: 'API endpoint not found'
    });
  });

  // ===== SPA Catch-all =====

  // For any other route, serve index.html (SPA)
  // This must be LAST so API routes are checked first
  app.get('*', (_req: Request, res: Response) => {
    res.sendFile(path.join(portalPath, 'index.html'));
  });

  // ===== Global Error Handler =====

  app.use((err: Error, _req: Request, res: Response, _next: NextFunction) => {
    console.error('[Error]', err);

    res.status(500).json({
      error: 'Internal server error'
    });
  });

  return app;
}

/**
 * Start server
 */
export function startServer(port: number = 5000): void {
  const app = createApp();

  app.listen(port, () => {
    console.log(`\n╔═══════════════════════════════════════════════════════════╗`);
    console.log(`║  TWZRD Gateway - Portal v3 Ready                          ║`);
    console.log(`╠═══════════════════════════════════════════════════════════╣`);
    console.log(`║  Server running at http://localhost:${port}`);
    console.log(`║  Portal v3: http://localhost:${port}`);
    console.log(`║  API: http://localhost:${port}/api/`);
    console.log(`║  Metrics: http://localhost:${port}/metrics`);
    console.log(`╠═══════════════════════════════════════════════════════════╣`);
    console.log(`║  Endpoints:`);
    console.log(`║  • GET  /api/verification-status?wallet=<pubkey>`);
    console.log(`║  • POST /api/claim-cls`);
    console.log(`║  • GET  /metrics (Prometheus)`);
    console.log(`╚═══════════════════════════════════════════════════════════╝\n`);

    // ===== Periodic Metrics Updates =====

    // Update epoch and viewer metrics every 60 seconds
    const updateMetrics = async () => {
      try {
        await updateEpochMetrics(db);
        await updateViewerMetrics(db);
      } catch (error) {
        console.error('[Metrics] Error in periodic update:', error);
      }
    };

    // Initial update
    updateMetrics();

    // Schedule periodic updates
    setInterval(updateMetrics, 60_000); // Every 60 seconds
    console.log('[Metrics] Periodic updates started (60s interval)');
  });
}

export default createApp;
