import unittest
from unittest.mock import patch
from service import Service
from tests.test_commerce import fixture

class GuestTests(unittest.TestCase):
    def test_checkout_does_not_require_an_account(self):
        service, handle, _ = fixture()
        with patch('official.checkout', return_value={'saleId': 'sale1', 'payment': {'amount': 4000}}) as checkout:
            result = service.run('checkout', {'session': handle, 'confirmedAmount': 4000,
                                            'email': 'test@example.org', 'phone': '912345678'})
        self.assertEqual(result['saleId'], 'sale1')
        self.assertIsNone(checkout.call_args.args[4])

    def test_guest_can_recover_a_sale_reference(self):
        service = Service()
        with patch('official.sale_status', return_value={'status': 'paid', 'isLoaded': False}) as status:
            result = service.run('recover', {'saleId': 'sale1', 'reader': 'TEST'})
        self.assertEqual(service.sessions.get(result['session'])['purchase']['saleId'], 'sale1')
        status.assert_called_once_with('sale1', None)

if __name__ == '__main__':
    unittest.main()
