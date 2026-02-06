/**
 * Simple development server for the Quantum Betting Platform
 * Serves static files and provides CORS headers for API testing
 */

const http = require('http');
const fs = require('fs');
const path = require('path');
const url = require('url');

const PORT = 8080;
const BASE_DIR = path.resolve(__dirname);
const MIME_TYPES = {
    '.html': 'text/html',
    '.js': 'application/javascript',
    '.css': 'text/css',
    '.json': 'application/json',
    '.png': 'image/png',
    '.jpg': 'image/jpeg',
    '.gif': 'image/gif',
    '.svg': 'image/svg+xml',
    '.ico': 'image/x-icon'
};

// Create server
const server = http.createServer((req, res) => {
    console.log(`${req.method} ${req.url}`);

    // Parse URL
    const parsedUrl = url.parse(req.url);
    let pathname = parsedUrl.pathname || '/';

    // Default to index.html
    if (pathname === '/') {
        pathname = '/platform_ui.html';
    }

    // Resolve path relative to this directory (not the process CWD) and block traversal.
    const filePath = path.resolve(BASE_DIR, `.${pathname}`);
    if (!filePath.startsWith(`${BASE_DIR}${path.sep}`)) {
        res.writeHead(400, { 'Content-Type': 'text/plain' });
        res.end('Bad Request');
        return;
    }

    // Get file extension
    const ext = path.extname(filePath);

    // Set CORS headers
    res.setHeader('Access-Control-Allow-Origin', '*');
    res.setHeader('Access-Control-Allow-Methods', 'GET, POST, OPTIONS');
    res.setHeader('Access-Control-Allow-Headers', 'Content-Type');

    // Handle OPTIONS requests
    if (req.method === 'OPTIONS') {
        res.writeHead(200);
        res.end();
        return;
    }

    // Read file
    fs.readFile(filePath, (err, data) => {
        if (err) {
            // File not found
            if (err.code === 'ENOENT') {
                res.writeHead(404, { 'Content-Type': 'text/html' });
                res.end('<h1>404 - File Not Found</h1>', 'utf-8');
            } else {
                // Server error
                res.writeHead(500);
                res.end(`Server Error: ${err.code}`, 'utf-8');
            }
        } else {
            // Success
            res.writeHead(200, { 'Content-Type': MIME_TYPES[ext] || 'text/plain' });
            res.end(data);
        }
    });
});

// Start server
server.listen(PORT, () => {
    console.log(`
╔══════════════════════════════════════════════╗
║     Quantum Betting Platform Dev Server      ║
╠══════════════════════════════════════════════╣
║                                              ║
║  Server running at:                          ║
║  http://localhost:${PORT}                       ║
║                                              ║
║  Available endpoints:                        ║
║  - http://localhost:${PORT}/                    ║
║  - http://localhost:${PORT}/platform_ui.html    ║
║                                              ║
║  Press Ctrl+C to stop the server             ║
║                                              ║
╚══════════════════════════════════════════════╝
    `);
});
