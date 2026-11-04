"""Short-lived process memory; no database, cookies, files or browser storage."""
import secrets
import threading
import time

class Sessions:
    def __init__(self, ttl=900):
        self.ttl, self.items, self.lock = ttl, {}, threading.RLock()

    def create(self, data):
        with self.lock:
            now = time.monotonic()
            self.items = {k: v for k, v in self.items.items() if v['expires'] > now}
            token = secrets.token_urlsafe(32)
            self.items[token] = {'expires': now + self.ttl, **data}
            return token

    def get(self, token):
        if not isinstance(token, str):
            return None
        with self.lock:
            item = self.items.get(token)
            if item and item['expires'] > time.monotonic():
                return item
            self.items.pop(token, None)
            return None
