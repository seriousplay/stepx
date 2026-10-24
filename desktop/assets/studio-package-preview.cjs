// Fixed preview server for the same self-contained HTML used in local tests.
// Expose only the entry document; no file browsing, shell, or runtime installs.
const fs = require('node:fs');
const http = require('node:http');
const port = Number(process.env.PORT || 5173);
if (!Number.isInteger(port) || port < 1024 || port > 65535) throw Error('Invalid preview port');
http.createServer((request, response) => {
  const url = new URL(request.url, 'http://localhost');
  if (!['GET', 'HEAD'].includes(request.method) || !['/', '/index.html'].includes(url.pathname)) {
    response.writeHead(404, { 'Content-Type': 'text/plain' });
    response.end('Not found');
    return;
  }
  const html = fs.readFileSync('index.html');
  response.writeHead(200, { 'Content-Type': 'text/html; charset=utf-8', 'Cache-Control': 'no-store', 'Content-Length': html.length });
  response.end(request.method === 'HEAD' ? undefined : html);
}).listen(port, '127.0.0.1', () => console.log('Local: http://localhost:' + port + '/'));
