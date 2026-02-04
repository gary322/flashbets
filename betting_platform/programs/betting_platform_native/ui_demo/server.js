const http = require('http');
const fs = require('fs');
const path = require('path');

const PORT = 8080;

const mimeTypes = {
    '.html': 'text/html',
    '.css': 'text/css',
    '.js': 'text/javascript',
    '.png': 'image/png',
    '.jpg': 'image/jpeg',
    '.gif': 'image/gif',
    '.svg': 'image/svg+xml',
    '.ico': 'image/x-icon'
};

const server = http.createServer((req, res) => {
    let filePath = req.url === '/' ? '/index.html' : req.url;
    filePath = path.join(__dirname, filePath);

    const extname = path.extname(filePath);
    const contentType = mimeTypes[extname] || 'text/plain';

    fs.readFile(filePath, (err, content) => {
        if (err) {
            if (err.code === 'ENOENT') {
                res.writeHead(404);
                res.end('File not found');
            } else {
                res.writeHead(500);
                res.end('Server error');
            }
        } else {
            res.writeHead(200, { 'Content-Type': contentType });
            res.end(content, 'utf-8');
        }
    });
});

server.listen(PORT, () => {
    console.log(`
🚀 Quantum Betting Platform UI Demo Server Running!
==================================================

Server is running at: http://localhost:${PORT}

📱 Available Pages:
------------------
• Landing Page:        http://localhost:${PORT}/index.html
• Preview (All Pages): http://localhost:${PORT}/preview.html
• Dashboard:          http://localhost:${PORT}/app/dashboard.html
• Create Market:      http://localhost:${PORT}/app/create-market.html
• Verse Management:   http://localhost:${PORT}/app/verses.html
• Markets Browser:    http://localhost:${PORT}/app/markets.html
• Trading Terminal:   http://localhost:${PORT}/app/trading.html
• Portfolio:          http://localhost:${PORT}/app/portfolio.html
• DeFi Hub:          http://localhost:${PORT}/app/defi.html

✨ Key Features:
----------------
• Users can add verses to markets (Step 2 of market creation)
• Professional blue color scheme
• Complete UI with all features
• Native Solana integration ready

Press Ctrl+C to stop the server
    `);
});