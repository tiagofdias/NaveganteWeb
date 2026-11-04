"""Origin-restricted, memory-only access from the hosted UI to local PC/SC."""
import secrets
from urllib.parse import urlsplit

TOKEN_HEADER = 'X-Navegante-Token'

class BridgeAccess:
    def __init__(self, origin='', token=None):
        origin = origin.rstrip('/')
        if origin:
            url = urlsplit(origin)
            if (url.scheme != 'https' or not url.hostname or url.username or url.password
                    or url.path or url.query or url.fragment or '*' in origin):
                raise ValueError('O domínio autorizado deve ser uma origem HTTPS exata, sem caminhos ou wildcards.')
        self.origin = origin
        self.token = token if token is not None else secrets.token_urlsafe(32)

    def allows_origin(self, origin):
        return bool(self.origin) and origin == self.origin

    def authenticated(self, headers):
        supplied = headers.get(TOKEN_HEADER, '')
        return (self.allows_origin(headers.get('Origin')) and isinstance(supplied, str)
                and supplied.isascii() and secrets.compare_digest(supplied, self.token))
