"""Local web app, Python standard library only; no database."""
import json
import argparse
import os
import threading
from http.server import BaseHTTPRequestHandler, HTTPServer, ThreadingHTTPServer
from pathlib import Path
from pcsc import PCSC, CardError
from tiptop import IntegrationError
from service import Service, ServiceError
from official import AccountError
from bridge import BridgeAccess, TOKEN_HEADER

HOST, PORT = '127.0.0.1', 8765
ROOT = Path(__file__).parent / 'web'
APP = Service()
APP_LOCK = threading.Lock()
OPERATIONS = {'inspect', 'read', 'catalog', 'product', 'prepare', 'checkout', 'payment', 'load',
              'recover'}

class Handler(BaseHTTPRequestHandler):
    def setup(self):
        super().setup()
        self.connection.settimeout(10)

    def log_message(self, format, *args):
        print(f"[HTTP] {getattr(self, 'command', '?')} {getattr(self, 'path', '?')} - {format % args}", flush=True)

    def reply(self, status, data, content_type='application/json; charset=utf-8'):
        body = json.dumps(data, ensure_ascii=False).encode() if not isinstance(data, bytes) else data
        self.send_response(status)
        self.send_header('Content-Type', content_type)
        self.send_header('Content-Length', str(len(body)))
        self.send_header('Cache-Control', 'no-store')
        access = getattr(self.server, 'bridge', None)
        if access and access.allows_origin(self.headers.get('Origin')):
            self.send_header('Access-Control-Allow-Origin', access.origin)
            self.send_header('Vary', 'Origin')
        self.send_header('Referrer-Policy', 'no-referrer')
        self.send_header('X-Content-Type-Options', 'nosniff')
        self.send_header('Content-Security-Policy', "default-src 'self'; frame-ancestors 'none'; base-uri 'none'; form-action 'self'")
        self.end_headers()
        self.wfile.write(body)

    def valid_request(self):
        expected = f'{HOST}:{self.server.server_port}'
        if self.headers.get('Host') != expected:
            return False
        access = getattr(self.server, 'bridge', None)
        if access and access.allows_origin(self.headers.get('Origin')):
            return self.path.startswith('/api/') and access.authenticated(self.headers)
        return (self.headers.get('Host') == expected
                and self.headers.get('Sec-Fetch-Site') != 'cross-site'
                and self.headers.get('Origin') in (None, f'http://{expected}'))

    def do_OPTIONS(self):
        access = getattr(self.server, 'bridge', None)
        allowed_paths = {'/api/readers', '/api/bridge'} | {'/api/'+op for op in OPERATIONS}
        requested = {h.strip().lower() for h in self.headers.get('Access-Control-Request-Headers', '').split(',') if h.strip()}
        if (self.headers.get('Host') != f'{HOST}:{self.server.server_port}'
                or not access or not access.allows_origin(self.headers.get('Origin'))
                or self.path not in allowed_paths
                or self.headers.get('Access-Control-Request-Method') not in ('GET', 'POST')
                or not requested <= {'content-type', TOKEN_HEADER.lower()}):
            return self.reply(403, {'error': 'Origem ou pedido de ligação não autorizado.'})
        self.send_response(204)
        self.send_header('Access-Control-Allow-Origin', access.origin)
        self.send_header('Access-Control-Allow-Methods', 'GET, POST, OPTIONS')
        self.send_header('Access-Control-Allow-Headers', 'Content-Type, '+TOKEN_HEADER)
        self.send_header('Access-Control-Max-Age', '300')
        self.send_header('Vary', 'Origin')
        self.send_header('Content-Length', '0')
        self.end_headers()

    def do_GET(self):
        if not self.valid_request():
            return self.reply(403, {'error': 'Origem não autorizada.'})
        if self.path == '/api/bridge':
            return self.reply(200, {'service': 'navegante-local', 'version': 1})
        if self.path == '/api/readers':
            try:
                return self.reply(200, {'readers': PCSC().list_readers()})
            except (CardError, OSError) as exc:
                return self.reply(503, {'error': str(exc)})
        files = {'/': ('index.html', 'text/html; charset=utf-8'), '/app.js': ('app.js', 'text/javascript; charset=utf-8'), '/style.css': ('style.css', 'text/css; charset=utf-8'), '/forms.css': ('forms.css', 'text/css; charset=utf-8')}
        if self.path not in files:
            return self.reply(404, {'error': 'Não encontrado.'})
        file, mime = files[self.path]
        self.reply(200, (ROOT / file).read_bytes(), mime)

    def do_POST(self):
        if not self.valid_request() or not self.headers.get('Origin'):
            return self.reply(403, {'error': 'Origem não autorizada.'})
        operation = self.path.removeprefix('/api/')
        if self.path != '/api/' + operation or operation not in OPERATIONS:
            return self.reply(404, {'error': 'Não encontrado.'})
        if self.headers.get_content_type() != 'application/json':
            return self.reply(415, {'error': 'Usa JSON neste pedido.'})
        try:
            length = int(self.headers.get('Content-Length', '0'))
            if not 0 < length <= 8192:
                return self.reply(400, {'error': 'Pedido inválido.'})
            data = json.loads(self.rfile.read(length))
            if not isinstance(data, dict):
                return self.reply(400, {'error': 'Pedido inválido.'})
            with APP_LOCK:
                result = APP.run(operation, data)
            self.reply(200, result)
        except (ValueError, UnicodeError) as exc:
            print(f"[ERROR] 400 Pedido inválido: {exc}", flush=True)
            self.reply(400, {'error': 'Pedido inválido.'})
        except (ServiceError, AccountError) as exc:
            print(f"[ERROR] {exc.status} {exc}", flush=True)
            self.reply(exc.status, {'error': str(exc)})
        except (CardError, OSError) as exc:
            print(f"[ERROR] 409 {exc}", flush=True)
            self.reply(409, {'error': str(exc)})
        except IntegrationError as exc:
            print(f"[ERROR] 502 {exc}", flush=True)
            self.reply(502, {'error': str(exc)})

if __name__ == '__main__':
    parser = argparse.ArgumentParser(description='Navegante Web e ponte local PC/SC')
    parser.add_argument('--web-origin', default=os.environ.get('NAVEGANTE_WEB_ORIGIN', ''),
                        help='Origem HTTPS exata do site autorizado a usar este leitor')
    args = parser.parse_args()
    try:
        access = BridgeAccess(args.web_origin)
    except ValueError as exc:
        parser.error(str(exc))
    print(f'Navegante Web: http://{HOST}:{PORT}', flush=True)
    server = ThreadingHTTPServer((HOST, PORT), Handler)
    server.bridge = access
    if access.origin:
        print(f'Site autorizado: {access.origin}', flush=True)
        print(f'Código de ligação (válido só enquanto este processo estiver aberto): {access.token}', flush=True)
    server.allow_reuse_address = False
    server.serve_forever()
