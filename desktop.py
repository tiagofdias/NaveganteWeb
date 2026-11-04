"""Standalone Windows app: the same web UI, with local PC/SC and no terminal."""
import os
from pathlib import Path
import subprocess
import sys
import tempfile
import threading
import tkinter as tk
from tkinter import messagebox
from http.client import HTTPConnection
from http.server import ThreadingHTTPServer
import json
import server
from bridge import BridgeAccess

class QuietHandler(server.Handler):
    def log_message(self, *args):
        pass

def edge_path():
    for base in (os.environ.get('PROGRAMFILES(X86)'), os.environ.get('PROGRAMFILES'), os.environ.get('LOCALAPPDATA')):
        if base:
            candidate = Path(base) / 'Microsoft' / 'Edge' / 'Application' / 'msedge.exe'
            if candidate.is_file():
                return candidate
    raise RuntimeError('O Microsoft Edge não foi encontrado. Esta versão da aplicação necessita do Edge instalado no Windows.')

def main():
    executable = edge_path()
    http = ThreadingHTTPServer((server.HOST, 0), QuietHandler)
    http.bridge = BridgeAccess()  # Local application only: no hosted-origin access.
    thread = threading.Thread(target=http.serve_forever, daemon=True)
    thread.start()
    try:
        with tempfile.TemporaryDirectory(prefix='navegante-ui-', ignore_cleanup_errors=True) as profile:
            # This is a dedicated temporary Edge profile, never the user's own profile.
            resolved = Path(profile).resolve()
            if resolved.parent != Path(tempfile.gettempdir()).resolve() or not resolved.name.startswith('navegante-ui-'):
                raise RuntimeError('Não foi possível preparar a janela da aplicação.')
            child = subprocess.Popen([str(executable),
                f'--app=http://{server.HOST}:{http.server_port}',
                '--user-data-dir='+str(resolved), '--no-first-run', '--no-default-browser-check',
                '--disable-background-mode', '--window-size=1280,900'],
                creationflags=getattr(subprocess, 'CREATE_NO_WINDOW', 0))
            child.wait()
            # Let an in-progress card operation finish if the window was closed.
            with server.APP_LOCK:
                pass
    finally:
        http.shutdown()
        http.server_close()
        thread.join()

def self_test():
    """Packaged smoke test: UI runtime, bundled page and reader enumeration only."""
    edge_path()
    root = tk.Tk()
    root.withdraw()
    root.update()
    root.destroy()
    http = ThreadingHTTPServer((server.HOST, 0), QuietHandler)
    http.bridge = BridgeAccess()
    thread = threading.Thread(target=http.serve_forever, daemon=True)
    thread.start()
    try:
        client = HTTPConnection(server.HOST, http.server_port, timeout=10)
        client.request('GET', '/')
        response = client.getresponse()
        assert response.status == 200
        page = response.read()
        assert b'data-hosted-demo="true"' not in page
        assert b'data-remote="true"' not in page
        client.close()
        client = HTTPConnection(server.HOST, http.server_port, timeout=10)
        client.request('GET', '/api/readers')
        response = client.getresponse()
        assert response.status in (200,503)
        if response.status == 200:
            assert isinstance(json.loads(response.read())['readers'], list)
        else:
            response.read()
        client.close()
    finally:
        http.shutdown()
        http.server_close()
        thread.join()

if __name__ == '__main__':
    if '--self-test' in sys.argv:
        try:
            self_test()
        except Exception:
            sys.exit(1)
    else:
        try:
            main()
        except Exception as exc:
            root = tk.Tk()
            root.withdraw()
            messagebox.showerror('Navegante Web', str(exc), parent=root)
            root.destroy()
            sys.exit(1)
