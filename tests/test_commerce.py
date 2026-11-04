import time
import unittest
from contextlib import contextmanager
from unittest.mock import patch
from commerce import prepare, price_amount
from sessions import Sessions
from service import Service
from tiptop import IntegrationError

def fixture():
    service = Service()
    session = {'reader': 'TEST', 'read_id': '123', 'result': {'data': {'sessionId': 'official-session'}},
               'quote': {'amount': 4000, 'created': time.monotonic()}}
    handle = service.sessions.create(session)
    auth = None
    return service, handle, auth

class CommerceTests(unittest.TestCase):
    def test_expired_sessions_are_not_reused(self):
        sessions = Sessions(ttl=-1)
        self.assertIsNone(sessions.get(sessions.create({'secret': 'example'})))

    def test_price_uses_integer_cents(self):
        self.assertEqual(price_amount({'net': '3774', 'tax': '226'}), 4000)
        for bad in ('NaN', '-1', '0.1'):
            with self.assertRaises(IntegrationError):
                price_amount({'net': bad, 'tax': '0'})

    def test_quote_does_not_accept_prices_from_browser(self):
        calls = []
        session = {'read_id': 'read1', 'products': {'metro': {'id': 'config1', 'name': 'Metro',
            'attributes': [{'id': 'product_monthly', 'values': [{'id': 'variant1', 'value': '11'}]}],
            'prices': [{'id': 'variant1', 'net': '3774', 'tax': '226', 'tax_factor': '600', 'quantity': '1'}]}}}
        quote = prepare(session, 'metro', 'variant1', lambda *args: calls.append(args))
        self.assertEqual(quote['amount'], 4000)
        self.assertIs(type(calls[0][2]['prices'][0]['net']), int)
        self.assertEqual(calls[0][2]['attributes'][0]['id'], 'variant1')
        self.assertNotIn('sequence', calls[0][2]['attributes'][0])
        self.assertNotIn('description', calls[0][2]['prices'][0])
        with self.assertRaises(IntegrationError):
            prepare(session, 'metro', 'browser-price', lambda *_: self.fail('Must not call server'))

    def test_checkout_requires_confirmed_price(self):
        service, handle, auth = fixture()
        with patch('official.checkout') as checkout:
            for data in ({'session': handle}, {'session': handle, 'auth': auth, 'confirmedAmount': 3}):
                with self.assertRaises(IntegrationError):
                    service.run('checkout', data)
            checkout.assert_not_called()

    def test_checkout_timeout_blocks_duplicate_submission(self):
        service, handle, auth = fixture()
        data = {'session': handle, 'auth': auth, 'confirmedAmount': 4000, 'email': 'test@example.org', 'phone': '912345678'}
        with patch('official.checkout', side_effect=IntegrationError('timeout')) as checkout:
            for _ in range(2):
                with self.assertRaises(IntegrationError):
                    service.run('checkout', data)
            self.assertEqual(checkout.call_count, 1)
        self.assertEqual(service.sessions.get(handle)['purchase']['state'], 'unknown')

    def test_load_requires_paid_confirmation_from_official_service(self):
        service, handle, auth = fixture()
        service.sessions.get(handle)['purchase'] = {'saleId': 'sale1', 'state': 'pending'}
        with patch('official.sale_status', return_value={'status': 'pending_payment', 'isLoaded': False}), \
             patch('official.payment_status', return_value={'status': 'pending_payment', 'loadRequest': None}), \
             patch('service.PCSC') as pcsc:
            with self.assertRaises(IntegrationError):
                service.run('load', {'session': handle, 'auth': auth, 'confirmWrite': True})
            pcsc.assert_not_called()

    def test_interrupted_write_cannot_be_replayed(self):
        service, handle, auth = fixture()
        session = service.sessions.get(handle)
        session['purchase'] = {'saleId': 'sale1', 'state': 'paid'}
        signed = {'request64': 'from-official-service', 'keyRef': 'key', 'signature': 'signature'}
        @contextmanager
        def connection(_):
            yield lambda command: b'\x90\0'
        class Reader:
            connect = staticmethod(connection)
        def fail_write(transmit, request, progress):
            progress(bytes.fromhex('94DC01440100'))
            raise IntegrationError('network lost after write')
        with patch('official.sale_status', return_value={'status': 'paid', 'isLoaded': False}), \
             patch('official.payment_status', return_value={'status': 'paid', 'loadRequest': signed}), \
             patch('service.PCSC', return_value=Reader()), patch('service.load_card', side_effect=fail_write) as load:
            for _ in range(2):
                with self.assertRaises(IntegrationError):
                    service.run('load', {'session': handle, 'auth': auth, 'confirmWrite': True})
            self.assertEqual(load.call_count, 1)
        self.assertEqual(session['purchase']['state'], 'uncertain')

    def test_already_loaded_does_not_touch_card(self):
        service, handle, auth = fixture()
        service.sessions.get(handle)['purchase'] = {'saleId': 'sale1', 'state': 'uncertain'}
        with patch('official.sale_status', return_value={'status': 'paid', 'isLoaded': True}), patch('service.PCSC') as pcsc:
            result = service.run('load', {'session': handle, 'auth': auth})
            self.assertEqual(result['state'], 'loaded')
            pcsc.assert_not_called()

    def test_full_checkout_to_verified_load_with_mocked_official_services(self):
        service, handle, auth = fixture()
        data = {'session': handle, 'auth': auth, 'confirmedAmount': 4000, 'email': 'test@example.org', 'phone': '912345678'}
        signed = {'request64': 'signed-by-official-service', 'signature': 'mac', 'keyRef': 'key'}
        @contextmanager
        def connection(_):
            yield lambda command: b'\x90\0'
        class Reader:
            connect = staticmethod(connection)
        statuses = [{'status': 'paid', 'isLoaded': False}, {'status': 'paid', 'isLoaded': False}, {'status': 'paid', 'isLoaded': True}]
        with patch('official.checkout', return_value={'saleId': 'sale1', 'payment': {'amount': 4000}}), \
             patch('official.sale_status', side_effect=statuses), \
             patch('official.payment_status', return_value={'status': 'paid', 'loadRequest': signed}), \
             patch('service.PCSC', return_value=Reader()), patch('service.load_card') as load:
            self.assertEqual(service.run('checkout', data)['saleId'], 'sale1')
            self.assertTrue(service.run('payment', {'session': handle, 'auth': auth})['canLoad'])
            result = service.run('load', {'session': handle, 'auth': auth, 'confirmWrite': True})
            self.assertEqual(result['state'], 'loaded')
            self.assertEqual(load.call_args.args[1], signed)
            self.assertNotIn('load_request', service.sessions.get(handle))

if __name__ == '__main__':
    unittest.main()
