"""One-click Windows companion. No terminal, Python install or typed pairing code."""
import queue
import threading
import tkinter as tk
from tkinter import messagebox
import webbrowser
import sys
from http.client import HTTPConnection
import json
from http.server import ThreadingHTTPServer
import server
from bridge import BridgeAccess

SITE = 'https://navegante-web-xi.vercel.app'

class Approval:
    def __init__(self):
        self.requests = queue.Queue(maxsize=1)
        self.lock = threading.Lock()

    def request(self):
        if not self.lock.acquire(blocking=False):
            return False
        try:
            event = threading.Event()
            result = {'allowed': False, 'expired': False}
            try:
                self.requests.put_nowait((event, result))
            except queue.Full:
                return False
            if not event.wait(65):
                result['expired'] = True
                return False
            return result['allowed']
        finally:
            self.lock.release()

class QuietHandler(server.Handler):
    def log_message(self, *args):
        pass

def main():
    root = tk.Tk()
    root.title('Navegante Link')
    root.geometry('480x365')
    root.resizable(False, False)
    root.configure(bg='#101416')
    title = tk.Label(root, text='navegante  /  LINK', bg='#101416', fg='#ffdf00', font=('Segoe UI', 23, 'bold'))
    title.pack(anchor='w', padx=30, pady=(28, 18))
    tk.Label(root, text='O teu leitor, ligado ao site.', bg='#101416', fg='#f3f5ef', font=('Segoe UI', 16, 'bold')).pack(anchor='w', padx=30)
    status = tk.StringVar(value='A iniciar…')
    tk.Label(root, textvariable=status, wraplength=410, justify='left', bg='#101416', fg='#aeb9bf', font=('Segoe UI', 11)).pack(anchor='w', padx=30, pady=17)
    tk.Button(root, text='Abrir Navegante Web  ↗', command=lambda:webbrowser.open(SITE), bg='#ffdf00', fg='#14180c', activebackground='#ffe94b', relief='flat', padx=20, pady=12, font=('Segoe UI', 11, 'bold')).pack(anchor='w', padx=30, pady=7)
    tk.Label(root, text='Mantém esta janela aberta enquanto usas o passe.\nProjeto independente, sem afiliação à TML.', justify='left', bg='#101416', fg='#78868c', font=('Segoe UI', 9)).pack(anchor='w', padx=30, pady=18)
    approval = Approval()
    try:
        http = ThreadingHTTPServer((server.HOST, server.PORT), QuietHandler)
    except OSError:
        status.set('Já existe uma aplicação a usar o leitor. Fecha a versão anterior e volta a abrir o Navegante Link.')
        root.mainloop()
        return
    http.bridge = BridgeAccess(SITE, approve=approval.request)
    threading.Thread(target=http.serve_forever, daemon=True).start()
    status.set('Pronto. Liga o leitor USB, abre o site e escolhe «Ligar leitor». Não precisas de códigos.')

    def poll():
        try:
            event, result = approval.requests.get_nowait()
        except queue.Empty:
            pass
        else:
            if not result['expired']:
                root.deiconify()
                root.lift()
                result['allowed'] = messagebox.askyesno('Autorizar Navegante Web',
                    'Permitir que o site Navegante Web utilize o leitor deste PC?\n\n'+SITE+
                    '\n\nPagamentos e carregamentos continuam a exigir a tua confirmação no site.', parent=root)
                status.set('Site ligado. Podes ler o passe no navegador.' if result['allowed'] else 'Ligação recusada. Podes tentar novamente no site.')
            event.set()
        root.after(150, poll)

    def close():
        if server.APP_LOCK.locked():
            messagebox.showinfo('Operação em curso', 'Aguarda que a operação no cartão termine antes de fechar.', parent=root)
            return
        http.shutdown()
        http.server_close()
        root.destroy()
    root.protocol('WM_DELETE_WINDOW', close)
    root.after(150, poll)
    root.mainloop()

def self_test():
    """Packaged smoke test: UI runtime, local handshake and reader enumeration only."""
    root = tk.Tk()
    root.withdraw()
    root.update()
    root.destroy()
    http = ThreadingHTTPServer((server.HOST, 0), QuietHandler)
    http.bridge = BridgeAccess('https://package-test.example', approve=lambda:True)
    thread = threading.Thread(target=http.serve_forever, daemon=True)
    thread.start()
    try:
        client = HTTPConnection(server.HOST, http.server_port, timeout=10)
        client.request('POST', '/api/pair', json.dumps({'client':'navegante-web'}),
                       {'Origin':http.bridge.origin, 'Content-Type':'application/json'})
        response = client.getresponse()
        assert response.status == 200
        token = json.loads(response.read())['token']
        client.close()
        client = HTTPConnection(server.HOST, http.server_port, timeout=10)
        client.request('GET', '/api/readers', headers={'Origin':http.bridge.origin,'X-Navegante-Token':token})
        response = client.getresponse()
        assert response.status in (200,503)  # No reader/service installed is a handled state.
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
        main()
