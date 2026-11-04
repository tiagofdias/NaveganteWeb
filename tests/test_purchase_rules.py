import unittest
from datetime import date
from unittest.mock import Mock
from commerce import prepare
from purchase_rules import purchase_block, annotate_offers
from tiptop import IntegrationError

METRO = 'id-prod-navegante-metro'
LISBOA = 'id-prod-navegante-lisboa'

def card(product=METRO, month=11, year=2026, status='1'):
    return {'contracts': [{'product_detail': {'product': {'id': product, 'name': product}},
                          'contract': [{'id': 'contract.temporal_validity.'+key, 'value_id': str(value)}
                                       for key, value in [('month', month), ('year', year), ('status', status)]]}]}

def details(month=11):
    return {'id': 'config', 'name': 'Passe', 'attributes': [{'id': 'product_monthly',
            'values': [{'id': 'choice', 'value': str(month)}]}],
            'prices': [{'id': 'choice', 'net': '4000', 'tax': '0', 'tax_factor': '0', 'quantity': '1'}]}

class PurchaseRulesTests(unittest.TestCase):
    def blocked(self, loaded, product=METRO, month=11, today=date(2026, 11, 3)):
        return purchase_block(loaded, product, details(month), 'choice', today)

    def test_same_month_metro_and_municipal_are_blocked(self):
        for product in (METRO, LISBOA):
            self.assertTrue(self.blocked(card(), product))

    def test_other_month_is_available(self):
        self.assertIsNone(self.blocked(card(), month=12))

    def test_monthly_card_with_only_expiry_date(self):
        loaded = {'contracts': [{'contract': [
            {'id': 'contract.name', 'value': 'navegante® Metropolitano'},
            {'id': 'contract.temporal_validity.date', 'value_id': '2026-11-30'},
            {'id': 'contract.temporal_validity.status', 'value_id': '1'}]}]}
        self.assertTrue(self.blocked(loaded, LISBOA))
        self.assertTrue(self.blocked(loaded, METRO))
        self.assertIsNone(self.blocked(loaded, LISBOA, month=12))

    def test_future_month_already_loaded_is_blocked(self):
        self.assertTrue(self.blocked(card(month=12), month=12))

    def test_municipal_does_not_block_metropolitan_upgrade(self):
        self.assertIsNone(self.blocked(card(LISBOA)))
        self.assertTrue(self.blocked(card(LISBOA), LISBOA))

    def test_unknown_or_expired_period_does_not_guess(self):
        self.assertIsNone(self.blocked(card(status='3')))
        self.assertIsNone(self.blocked(card(year='unknown')))
        self.assertIsNone(self.blocked({}))

    def test_balance_and_complement_are_preserved(self):
        for product in ('id-prod-zapping', 'id-prod-navegante-complemento'):
            self.assertIsNone(self.blocked(card(), product))
        balance = {'attributes': [{'id': 'product_occasional', 'values': [{'id': 'choice', 'value': '1000'}]}]}
        self.assertIsNone(purchase_block(card(), METRO, balance, 'choice'))

    def test_december_january_rollover(self):
        self.assertTrue(self.blocked(card(month=1, year=2027), month=1, today=date(2026, 12, 28)))
        self.assertIsNone(self.blocked(card(month=1, year=2026), month=1, today=date(2026, 12, 28)))

    def test_explicit_year_is_respected(self):
        self.assertTrue(self.blocked(card(), month='2026-11'))
        self.assertIsNone(self.blocked(card(), month='2027-11'))

    def test_annotation_does_not_mutate_official_data(self):
        original = details(date.today().month)
        annotated = annotate_offers(card(month=date.today().month, year=date.today().year), METRO, original)
        self.assertTrue(annotated['prices'][0]['blockedReason'])
        self.assertNotIn('blockedReason', original['prices'][0])

    def test_prepare_blocks_before_official_configuration(self):
        today = date.today()
        session = {'read_id': 'test', 'result': {'data': card(month=today.month, year=today.year)},
                   'products': {METRO: details(today.month)}}
        call = Mock()
        with self.assertRaises(IntegrationError):
            prepare(session, METRO, 'choice', call)
        call.assert_not_called()

if __name__ == '__main__':
    unittest.main()
