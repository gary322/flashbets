import type { NextApiRequest, NextApiResponse } from 'next';

const HOP_BY_HOP_HEADERS = new Set([
  'connection',
  'keep-alive',
  'proxy-authenticate',
  'proxy-authorization',
  'te',
  'trailer',
  'transfer-encoding',
  'upgrade',
]);

function getProxyTarget(): string {
  return (
    process.env.API_PROXY_TARGET ||
    process.env.API_BASE_URL ||
    process.env.NEXT_PUBLIC_API_URL ||
    'http://localhost:8081'
  );
}

export default async function handler(req: NextApiRequest, res: NextApiResponse) {
  try {
    const targetBaseUrl = getProxyTarget();

    const pathParam = req.query.path;
    const pathParts = Array.isArray(pathParam)
      ? pathParam
      : typeof pathParam === 'string'
        ? [pathParam]
        : [];

    const targetUrl = new URL(`/api/${pathParts.join('/')}`, targetBaseUrl);

    for (const [key, value] of Object.entries(req.query)) {
      if (key === 'path' || value === undefined) continue;

      if (Array.isArray(value)) {
        for (const entry of value) {
          targetUrl.searchParams.append(key, entry);
        }
      } else {
        targetUrl.searchParams.append(key, value);
      }
    }

    const headers: Record<string, string> = {};
    for (const [key, value] of Object.entries(req.headers)) {
      if (value === undefined) continue;

      const lowerKey = key.toLowerCase();
      if (HOP_BY_HOP_HEADERS.has(lowerKey) || lowerKey === 'host') continue;

      headers[key] = Array.isArray(value) ? value.join(',') : value;
    }

    const method = req.method || 'GET';
    const init: RequestInit = { method, headers };

    if (!['GET', 'HEAD'].includes(method)) {
      if (req.body !== undefined) {
        if (typeof req.body === 'string' || Buffer.isBuffer(req.body)) {
          init.body = req.body as any;
        } else {
          init.body = JSON.stringify(req.body);
          headers['content-type'] ||= 'application/json';
        }
      }
    }

    const upstreamResponse = await fetch(targetUrl, init);
    const body = Buffer.from(await upstreamResponse.arrayBuffer());

    res.status(upstreamResponse.status);

    upstreamResponse.headers.forEach((value, key) => {
      const lowerKey = key.toLowerCase();
      if (HOP_BY_HOP_HEADERS.has(lowerKey)) return;
      res.setHeader(key, value);
    });

    res.send(body);
  } catch (error) {
    console.error('API proxy error:', error);
    res.status(502).json({
      error: 'Bad gateway',
      details: error instanceof Error ? error.message : 'Unknown proxy error',
    });
  }
}

