#!/usr/bin/env node

/**
 * Polymarket mock server (demo/staging).
 *
 * Provides minimal endpoints used by the Rust API integration:
 * - GET  /health
 * - GET  /markets
 * - POST /orders
 * - GET  /orders
 * - GET  /orders/:order_id
 * - DELETE /orders/:order_id
 * - DELETE /orders/:order_id/cancel (compat)
 *
 * Notes:
 * - Does NOT validate auth headers or signatures (demo only).
 * - Stores orders in-memory.
 */

const http = require('http');
const { randomBytes, createHash } = require('crypto');
const { URL } = require('url');

const PORT = Number(process.env.POLYMARKET_MOCK_PORT || process.env.PORT || 8084);
const HOST = process.env.POLYMARKET_MOCK_HOST || '127.0.0.1';

function json(res, status, body) {
  const payload = Buffer.from(JSON.stringify(body));
  res.writeHead(status, {
    'content-type': 'application/json; charset=utf-8',
    'content-length': payload.length,
  });
  res.end(payload);
}

function notFound(res) {
  json(res, 404, { error: 'not_found' });
}

function badRequest(res, message) {
  json(res, 400, { error: 'bad_request', message });
}

function readBody(req) {
  return new Promise((resolve, reject) => {
    const chunks = [];
    req.on('data', (chunk) => chunks.push(chunk));
    req.on('end', () => resolve(Buffer.concat(chunks).toString('utf8')));
    req.on('error', reject);
  });
}

function nowIso() {
  return new Date().toISOString();
}

function newOrderId() {
  return `mock_order_${Date.now()}_${randomBytes(4).toString('hex')}`;
}

function newOrderHash(input) {
  const hex = createHash('sha256').update(input).digest('hex');
  return `0x${hex}`;
}

const mockMarkets = [
  {
    condition_id: 'mock_condition_1',
    question: 'Will the FlashBets demo pass CI?',
    description: 'Staging market served by the local Polymarket mock.',
    market_slug: 'flashbets-demo-ci',
    end_date_iso: '2026-12-31T00:00:00Z',
    game_start_time: null,
    active: true,
    closed: false,
    archived: false,
    accepting_orders: true,
    accepting_order_timestamp: null,
    minimum_order_size: 10.0,
    minimum_tick_size: 0.01,
    question_id: 'mock_q_1',
    seconds_delay: 0,
    fpmm: '0x0000000000000000000000000000000000000000',
    maker_base_fee: 0.0,
    taker_base_fee: 0.0,
    notifications_enabled: false,
    neg_risk: false,
    neg_risk_market_id: '',
    neg_risk_request_id: '',
    tokens: [
      { token_id: '1000001', outcome: 'Yes', price: 0.62, winner: false },
      { token_id: '1000002', outcome: 'No', price: 0.38, winner: false },
    ],
    icon: '',
    image: '',
    rewards: { rates: null, min_size: 1.0, max_spread: 0.1 },
    is_50_50_outcome: true,
    tags: ['demo', 'ci'],
    enable_order_book: true,
    category: 'Technology',
  },
  {
    condition_id: 'mock_condition_2',
    question: 'Will BTC be above $100k by end of 2026?',
    description: 'Demo crypto market.',
    market_slug: 'btc-100k-2026',
    end_date_iso: '2026-12-31T00:00:00Z',
    game_start_time: null,
    active: true,
    closed: false,
    archived: false,
    accepting_orders: true,
    accepting_order_timestamp: null,
    minimum_order_size: 10.0,
    minimum_tick_size: 0.01,
    question_id: 'mock_q_2',
    seconds_delay: 0,
    fpmm: '0x0000000000000000000000000000000000000000',
    maker_base_fee: 0.0,
    taker_base_fee: 0.0,
    notifications_enabled: false,
    neg_risk: false,
    neg_risk_market_id: '',
    neg_risk_request_id: '',
    tokens: [
      { token_id: '2000001', outcome: 'Yes', price: 0.41, winner: false },
      { token_id: '2000002', outcome: 'No', price: 0.59, winner: false },
    ],
    icon: '',
    image: '',
    rewards: { rates: null, min_size: 1.0, max_spread: 0.1 },
    is_50_50_outcome: true,
    tags: ['demo', 'crypto'],
    enable_order_book: true,
    category: 'Crypto',
  },
  {
    condition_id: 'mock_condition_3',
    question: 'Will the next.js trade flow submit an order?',
    description: 'E2E order submission should succeed against the mock CLOB.',
    market_slug: 'nextjs-order-flow',
    end_date_iso: '2026-12-31T00:00:00Z',
    game_start_time: null,
    active: true,
    closed: false,
    archived: false,
    accepting_orders: true,
    accepting_order_timestamp: null,
    minimum_order_size: 10.0,
    minimum_tick_size: 0.01,
    question_id: 'mock_q_3',
    seconds_delay: 0,
    fpmm: '0x0000000000000000000000000000000000000000',
    maker_base_fee: 0.0,
    taker_base_fee: 0.0,
    notifications_enabled: false,
    neg_risk: false,
    neg_risk_market_id: '',
    neg_risk_request_id: '',
    tokens: [
      { token_id: '3000001', outcome: 'Yes', price: 0.7, winner: false },
      { token_id: '3000002', outcome: 'No', price: 0.3, winner: false },
    ],
    icon: '',
    image: '',
    rewards: { rates: null, min_size: 1.0, max_spread: 0.1 },
    is_50_50_outcome: true,
    tags: ['demo', 'e2e'],
    enable_order_book: true,
    category: 'General',
  },
];

/** @type {Map<string, any>} */
const orders = new Map();

function toOrderResponse(orderId, orderPayload) {
  const sideValue = Number(orderPayload?.side ?? 0);
  const side = sideValue === 1 ? 'SELL' : 'BUY';
  const tokenId = String(orderPayload?.tokenId ?? orderPayload?.token_id ?? '0');

  const price = 0.5;
  const size = String(orderPayload?.takerAmount ?? orderPayload?.taker_amount ?? '0');

  const createdAt = nowIso();
  const orderHash = newOrderHash(`${orderId}:${tokenId}:${side}:${createdAt}`);

  return {
    order_id: orderId,
    order_hash: orderHash,
    status: 'OPEN',
    market_id: tokenId,
    outcome: side === 'BUY' ? 'YES' : 'NO',
    side,
    size,
    price,
    filled_amount: '0',
    remaining_amount: size,
    average_fill_price: null,
    created_at: createdAt,
    updated_at: createdAt,
  };
}

const server = http.createServer(async (req, res) => {
  try {
    const method = req.method || 'GET';
    const url = new URL(req.url || '/', `http://${req.headers.host || `${HOST}:${PORT}`}`);
    const pathname = url.pathname.replace(/\/+$/, '') || '/';

    if (method === 'GET' && pathname === '/health') {
      return json(res, 200, { status: 'ok' });
    }

    if (method === 'GET' && pathname === '/markets') {
      // Match multiple client expectations (CLOB and internal integrations).
      return json(res, 200, { data: mockMarkets });
    }

    const marketMatch = pathname.match(/^\/markets\/([^/]+)$/);
    if (method === 'GET' && marketMatch) {
      const marketId = decodeURIComponent(marketMatch[1]);
      const market = mockMarkets.find((m) => m.condition_id === marketId);
      if (!market) return notFound(res);
      return json(res, 200, market);
    }

    if (pathname === '/orders' && method === 'POST') {
      const bodyText = await readBody(req);
      let parsed;
      try {
        parsed = bodyText ? JSON.parse(bodyText) : {};
      } catch (e) {
        return badRequest(res, 'invalid_json');
      }

      const orderPayload = parsed?.order;
      if (!orderPayload) {
        return badRequest(res, 'missing_order');
      }

      const orderId = newOrderId();
      const response = toOrderResponse(orderId, orderPayload);
      orders.set(orderId, response);

      return json(res, 200, response);
    }

    if (pathname === '/orders' && method === 'GET') {
      return json(res, 200, Array.from(orders.values()));
    }

    const orderMatch = pathname.match(/^\/orders\/([^/]+)$/);
    if (orderMatch) {
      const orderId = decodeURIComponent(orderMatch[1]);

      if (method === 'GET') {
        const order = orders.get(orderId);
        if (!order) return notFound(res);
        return json(res, 200, order);
      }

      if (method === 'DELETE') {
        const existing = orders.get(orderId);
        if (existing) {
          existing.status = 'CANCELLED';
          existing.updated_at = nowIso();
          orders.set(orderId, existing);
        }
        return json(res, 200, {
          order_id: orderId,
          status: 'CANCELLED',
          cancelled_at: nowIso(),
        });
      }
    }

    const cancelCompat = pathname.match(/^\/orders\/([^/]+)\/cancel$/);
    if (cancelCompat && method === 'DELETE') {
      const orderId = decodeURIComponent(cancelCompat[1]);
      const existing = orders.get(orderId);
      if (existing) {
        existing.status = 'CANCELLED';
        existing.updated_at = nowIso();
        orders.set(orderId, existing);
      }
      return json(res, 200, {
        order_id: orderId,
        status: 'CANCELLED',
        cancelled_at: nowIso(),
      });
    }

    return notFound(res);
  } catch (err) {
    console.error('mock server error', err);
    return json(res, 500, { error: 'internal_error' });
  }
});

server.listen(PORT, HOST, () => {
  console.log(`[polymarket-mock] listening on http://${HOST}:${PORT}`);
});
