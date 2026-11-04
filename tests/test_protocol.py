import base64
import ctypes as c
import json
import unittest
from ctypes import wintypes as w
from pcsc import PCSC
from tiptop import IntegrationError, execute_read_commands, read_card, load_card

def batch(*commands):
    raw = b'\x02' + bytes([len(commands)])
    for command in commands:
        raw += b'\x49\x01' + len(command).to_bytes(4, 'big') + command
    return base64.b64encode(raw).decode()

class ProtocolTests(unittest.TestCase):
    def test_read_batch_encodes_responses(self):
        answer = execute_read_commands(batch(bytes.fromhex('94B201441D')), lambda _: b'\x12\x90\x00')
        self.assertEqual(answer, [2, 1, 73, 0, 0, 0, 0, 0, 0, 0, 3, 18, 144, 0])

    def test_validates_whole_batch_before_any_transmission(self):
        calls = []
        with self.assertRaises(IntegrationError):
            execute_read_commands(batch(bytes.fromhex('94B201441D'), bytes.fromhex('94DC01440100')), calls.append)
        self.assertEqual(calls, [])

    def test_rejects_truncation_and_wrong_count(self):
        for raw in (b'\x02\x01\x49\x01', b'\x02\x02', b'\x01\x00'):
            with self.assertRaises(IntegrationError):
                execute_read_commands(base64.b64encode(raw).decode(), lambda _: self.fail('Must not transmit'))

    def test_read_verification_is_not_allowed_without_authenticated_session(self):
        def api(*_):
            return {'id': '123', 'PO_command': batch(bytes.fromhex('942000000401020304'))}
        with self.assertRaises(IntegrationError):
            read_card(lambda _: self.fail('Must not transmit'), api)

    def test_signed_load_cannot_contain_an_arbitrary_path(self):
        signed = {'signature': 'server-signature', 'keyRef': 'server-key',
                  'request64': base64.b64encode(json.dumps({'method': 'POST', 'path': 'other'}).encode()).decode()}
        with self.assertRaises(IntegrationError):
            load_card(lambda _: self.fail('Must not transmit'), signed)

    def test_paid_load_protocol_only_calls_expected_resource(self):
        signed = {'signature': 'server-signature', 'keyRef': 'server-key',
                  'request64': base64.b64encode(json.dumps({'method': 'POST', 'path': 'loads'}).encode()).decode()}
        commands, calls = [], []
        def api(method, path, body):
            calls.append((method, path))
            return {'id': '456', 'PO_command': None, 'progress': 100}
        result = load_card(lambda apdu: commands.append(apdu) or b'\x90\0', signed,
                           call=api, signed_call=lambda _: {'id': '456', 'PO_command': batch(bytes.fromhex('94DC01440100'))})
        self.assertEqual(result['progress'], 100)
        self.assertEqual(calls, [('PUT', 'loads/456')])
        self.assertEqual(len(commands), 2)
        self.assertEqual(commands[0][1], 0xA4)

    def test_never_changes_pin_or_keys_even_when_loading(self):
        for instruction in (0xD8, 0x04, 0x44):
            with self.assertRaises(IntegrationError):
                execute_read_commands(batch(bytes([0x94, instruction, 0, 0, 4, 0, 0, 0, 0])),
                                      lambda _: self.fail('Must not transmit'), loading=True)

    def test_t0_open_session_is_not_replayed_on_6c(self):
        commands = []
        responses = [b'\x6c\x22', b'X' * 34 + b'\x90\x00']
        class DLL:
            def SCardTransmit(self, card, pci, tx, length, _, rx, rx_length):
                commands.append(bytes(tx[:length]))
                response = responses.pop(0)
                for i, byte in enumerate(response):
                    rx[i] = byte
                c.cast(rx_length, c.POINTER(w.DWORD)).contents.value = len(response)
                return 0
        pcsc = PCSC.__new__(PCSC)
        pcsc.dll = DLL()
        command = bytes.fromhex('948A89380401020304')
        self.assertEqual(len(pcsc.transmit(1, 1, command)), 36)
        self.assertEqual(commands, [command + b'\x00', bytes.fromhex('94C0000022')])

if __name__ == '__main__':
    unittest.main()
