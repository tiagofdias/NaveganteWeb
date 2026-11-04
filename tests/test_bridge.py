import http.client
import json
import threading
import unittest
from unittest.mock import patch
from http.server import HTTPServer
from bridge import BridgeAccess, TOKEN_HEADER
from server import Handler

class QuietHandler(Handler):
    def log_message(self, *args):
        pass

class BridgeTests(unittest.TestCase):
    def setUp(self):
        self.origin = 'https://navegante-test.vercel.app'
        self.token = 'test-pairing-code-' + 'x'*32
        self.server = HTTPServer(('127.0.0.1', 0), QuietHandler)
        self.server.bridge = BridgeAccess(self.origin, self.token)
        self.thread = threading.Thread(target=self.server.serve_forever, daemon=True)
        self.thread.start()

    def tearDown(self):
        self.server.shutdown()
        self.server.server_close()
        self.thread.join()

    def request(self, method='GET', path='/api/bridge', headers=None, body=None):
        connection = http.client.HTTPConnection('127.0.0.1', self.server.server_port)
        try:
            connection.request(method, path, body=body, headers=headers or {})
            response = connection.getresponse()
            return response.status, dict(response.headers), response.read()
        finally:
            connection.close()

    def paired(self, **extra):
        return {'Origin': self.origin, TOKEN_HEADER: self.token, 'Sec-Fetch-Site': 'cross-site', **extra}

    def test_exact_origin_and_token_are_required(self):
        for headers in ({'Origin': self.origin}, self.paired(**{TOKEN_HEADER:'wrong'}),
                        self.paired(Origin='https://evil.example'), self.paired(Origin='null')):
            self.assertEqual(self.request(headers=headers)[0], 403)
        status, headers, body = self.request(headers=self.paired())
        self.assertEqual(status, 200)
        self.assertEqual(headers['Access-Control-Allow-Origin'], self.origin)
        self.assertEqual(json.loads(body)['service'], 'navegante-local')
        self.assertNotIn(self.token.encode(), body)

    def test_only_authorized_preflight_gets_cors_permission(self):
        headers = {'Origin': self.origin, 'Access-Control-Request-Method':'POST',
                   'Access-Control-Request-Headers':'content-type,x-navegante-token',
                   'Access-Control-Request-Private-Network':'true'}
        status, reply, _ = self.request('OPTIONS', '/api/read', headers)
        self.assertEqual(status, 204)
        self.assertEqual(reply['Access-Control-Allow-Headers'], 'Content-Type, '+TOKEN_HEADER)
        self.assertEqual(reply['Access-Control-Allow-Origin'], self.origin)
        headers['Origin'] = 'https://evil.example'
        status, reply, _ = self.request('OPTIONS', '/api/read', headers)
        self.assertEqual(status, 403)
        self.assertNotIn('Access-Control-Allow-Origin', reply)

    def test_rejects_unexpected_host_methods_headers_and_paths(self):
        self.assertEqual(self.request(headers=self.paired(Host='evil.example'))[0], 403)
        for method, path, requested in [('DELETE','/api/read','content-type'),
                                        ('POST','/','content-type'),
                                        ('POST','/api/read','x-unexpected')]:
            headers = {'Origin':self.origin, 'Access-Control-Request-Method':method,
                       'Access-Control-Request-Headers':requested}
            self.assertEqual(self.request('OPTIONS',path,headers)[0],403)
        self.assertEqual(self.request(path='/',headers=self.paired())[0],403)

    def test_unauthorized_writes_never_reach_service(self):
        with patch('server.APP.run') as run:
            status, _, _ = self.request('POST','/api/load',{'Origin':self.origin,'Content-Type':'application/json'},'{}')
            self.assertEqual(status,403)
            run.assert_not_called()

    def test_pairing_requires_native_approval_and_exact_origin(self):
        approved = []
        self.server.bridge.approve = lambda: approved.append(True) or True
        headers = {'Origin': self.origin, 'Content-Type':'application/json'}
        status, _, body = self.request('POST','/api/pair',headers,'{"client":"navegante-web"}')
        self.assertEqual(status,200)
        self.assertEqual(json.loads(body)['token'],self.token)
        self.assertEqual(len(approved),1)
        headers['Origin']='https://evil.example'
        self.assertEqual(self.request('POST','/api/pair',headers,'{"client":"navegante-web"}')[0],403)
        self.assertEqual(len(approved),1)

    def test_pairing_denial_and_legacy_bridge_never_disclose_token(self):
        headers={'Origin':self.origin,'Content-Type':'application/json'}
        for approve, expected in ((lambda:False,403),(None,409)):
            self.server.bridge.approve=approve
            status, _, body=self.request('POST','/api/pair',headers,'{"client":"navegante-web"}')
            self.assertEqual(status,expected)
            self.assertNotIn(self.token.encode(),body)

    def test_authenticated_operations_use_existing_local_service(self):
        with patch('server.APP.run',return_value={'example':True}) as run:
            status, _, body = self.request('POST','/api/read',self.paired(**{'Content-Type':'application/json'}),'{"reader":"TEST"}')
            self.assertEqual(status,200)
            self.assertEqual(json.loads(body),{'example':True})
            run.assert_called_once_with('read',{'reader':'TEST'})

    def test_local_mode_still_works_without_pairing(self):
        self.server.bridge = BridgeAccess()
        origin = f'http://127.0.0.1:{self.server.server_port}'
        self.assertEqual(self.request(headers={'Origin':origin,'Sec-Fetch-Site':'same-origin'})[0],200)
        self.assertEqual(self.request(headers={'Origin':self.origin,TOKEN_HEADER:self.token})[0],403)

    def test_configuration_never_accepts_wildcards_or_insecure_remote_origins(self):
        for origin in ('*','https://*.vercel.app','http://example.com','https://example.com/path','https://user:pass@example.com'):
            with self.assertRaises(ValueError):
                BridgeAccess(origin)

if __name__ == '__main__':
    unittest.main()
