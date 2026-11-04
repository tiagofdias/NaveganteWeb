"""Windows PC/SC connection. No browser endpoint accepts arbitrary APDUs."""
import ctypes as c
from ctypes import wintypes as w
from contextlib import contextmanager

AIDS = {'legacy': '315449432E494341', 'lisbon': 'A000000291D62000019101'}

class CardError(Exception):
    pass

def check(result):
    code = result & 0xffffffff
    messages = {0x8010000C: 'Insere o passe no leitor.', 0x8010002E: 'Nenhum leitor ligado.',
                0x8010000B: 'O cartão está ocupado por outra aplicação. Fecha essa aplicação e tenta novamente.',
                0x80100017: 'O leitor foi desligado.', 0x80100069: 'O cartão foi removido.',
                0x8010001D: 'O serviço Smart Card não está disponível.'}
    if code:
        raise CardError(messages.get(code, f'Erro PC/SC: {code:08X}'))

class PCSC:
    def __init__(self):
        self.dll = c.WinDLL('winscard.dll')
        definitions = {
            'SCardEstablishContext': [w.DWORD, c.c_void_p, c.c_void_p, c.POINTER(c.c_size_t)],
            'SCardReleaseContext': [c.c_size_t],
            'SCardListReadersW': [c.c_size_t, w.LPCWSTR, w.LPWSTR, c.POINTER(w.DWORD)],
            'SCardConnectW': [c.c_size_t, w.LPCWSTR, w.DWORD, w.DWORD, c.POINTER(c.c_size_t), c.POINTER(w.DWORD)],
            'SCardReconnect': [c.c_size_t, w.DWORD, w.DWORD, w.DWORD, c.POINTER(w.DWORD)],
            'SCardDisconnect': [c.c_size_t, w.DWORD],
            'SCardStatusW': [c.c_size_t, w.LPWSTR, c.POINTER(w.DWORD), c.POINTER(w.DWORD), c.POINTER(w.DWORD), c.c_void_p, c.POINTER(w.DWORD)],
            'SCardTransmit': [c.c_size_t, c.c_void_p, c.c_void_p, w.DWORD, c.c_void_p, c.c_void_p, c.POINTER(w.DWORD)],
        }
        for name, args in definitions.items():
            fn = getattr(self.dll, name)
            fn.argtypes, fn.restype = args, c.c_long

    @contextmanager
    def context(self):
        ctx = c.c_size_t()
        check(self.dll.SCardEstablishContext(0, None, None, c.byref(ctx)))
        try:
            yield ctx
        finally:
            self.dll.SCardReleaseContext(ctx)

    def readers(self, ctx):
        count = w.DWORD()
        result = self.dll.SCardListReadersW(ctx, None, None, c.byref(count))
        if result & 0xffffffff == 0x8010002E:
            return []
        check(result)
        buf = c.create_unicode_buffer(count.value)
        check(self.dll.SCardListReadersW(ctx, None, buf, c.byref(count)))
        return [n for n in buf[:count.value].split('\0') if n]

    def list_readers(self):
        with self.context() as ctx:
            return self.readers(ctx)

    @contextmanager
    def connect(self, reader):
        with self.context() as ctx:
            if reader not in self.readers(ctx):
                raise CardError('Seleciona um leitor disponível.')
            card, protocol = c.c_size_t(), w.DWORD()
            check(self.dll.SCardConnectW(ctx, reader, 1, 3, c.byref(card), c.byref(protocol)))
            try:
                # Start from a known state; a removed connection may leave an
                # unfinished Calypso authentication session on a powered card.
                check(self.dll.SCardReconnect(card, 1, 3, 1, c.byref(protocol)))
                yield lambda command: self.transmit(card, protocol.value, command)
            finally:
                self.dll.SCardDisconnect(card, 1)

    def inspect(self, reader):
        # Require an explicit reader; never probe every card (e.g. a YubiKey).
        with self.context() as ctx:
            if reader not in self.readers(ctx):
                raise CardError('Seleciona um leitor disponível.')
            card, protocol = c.c_size_t(), w.DWORD()
            check(self.dll.SCardConnectW(ctx, reader, 2, 3, c.byref(card), c.byref(protocol)))
            try:
                name, size = c.create_unicode_buffer(512), w.DWORD(512)
                state, atr, atr_size = w.DWORD(), (c.c_ubyte * 36)(), w.DWORD(36)
                check(self.dll.SCardStatusW(card, name, c.byref(size), c.byref(state), c.byref(protocol), atr, c.byref(atr_size)))
                results = []
                for label, aid in AIDS.items():
                    data = bytes.fromhex(aid)
                    command = bytes.fromhex('00A40400') + bytes([len(data)]) + data
                    response = self.transmit(card, protocol.value, command)
                    results.append({'application': label, 'aid': aid, 'status': response[-2:].hex().upper(),
                                    'fci': response[:-2].hex().upper(), 'recognized': response[-2:] in (b'\x90\x00', b'\x62\x83')})
                    if results[-1]['recognized']:
                        break
                return {'reader': reader, 'protocol': f'T={protocol.value - 1}',
                        'atr': bytes(atr[:atr_size.value]).hex(' ').upper(), 'applications': results,
                        'recognized': any(r['recognized'] for r in results)}
            finally:
                self.dll.SCardDisconnect(card, 0)

    def transmit(self, card, protocol, command):
        class PCI(c.Structure):
            _fields_ = [('protocol', w.DWORD), ('length', w.DWORD)]
        def send(data):
            pci = PCI(protocol, c.sizeof(PCI))
            tx, rx, length = (c.c_ubyte * len(data)).from_buffer_copy(data), (c.c_ubyte * 258)(), w.DWORD(258)
            check(self.dll.SCardTransmit(card, c.byref(pci), tx, len(data), None, rx, c.byref(length)))
            response = bytes(rx[:length.value])
            if len(response) < 2:
                raise CardError('Resposta incompleta do cartão.')
            return response
        # Calypso contact T=0: SELECT without Le can return 9000 without FCI.
        # Request FCI explicitly; the card supplies its expected length with 6Cxx.
        case4 = (len(command) >= 5 and (len(command) == 6 + command[4]
                 or command[1] in (0xA4, 0x8A, 0x8E) and len(command) == 5 + command[4]))
        if case4 and len(command) == 5 + command[4]:
            command += b'\0'
        response = send(command)
        if response[-2] == 0x6C and len(command) >= 5:
            # A T=0 case-4 command may already have executed. Retrieve its
            # pending data; replaying OPEN SESSION would return 6985.
            corrected = (bytes([command[0], 0xC0, 0, 0, response[-1]]) if case4
                         else command[:-1] + response[-1:])
            response = send(corrected)
        # T=0 can return response data separately through GET RESPONSE.
        payload = b''
        for _ in range(8):
            if response[-2] != 0x61:
                return payload + response
            payload += response[:-2]
            followup = bytes([command[0], 0xC0, 0, 0, response[-1]])
            response = send(followup)
            if response[-2] == 0x6C:
                response = send(followup[:-1] + response[-1:])
        raise CardError('Limite de respostas do cartão excedido.')
