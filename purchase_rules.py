"""Prevent overlapping monthly purchases using the last official card read."""
import re
import unicodedata
from datetime import date

MUNICIPALITIES = ('alcochete', 'almada', 'amadora', 'barreiro', 'cascais', 'lisboa',
                  'loures', 'mafra', 'moita', 'montijo', 'odivelas', 'oeiras',
                  'palmela', 'seixal', 'sesimbra', 'setubal', 'sintra', 'vfxira')

def normalized(value):
    return ''.join(c for c in unicodedata.normalize('NFD', str(value or '').lower())
                   if unicodedata.category(c) != 'Mn').strip()

def monthly_kind(product_id, name):
    if product_id == 'id-prod-navegante-metro' or 'metropolitano' in normalized(name):
        return 'metro'
    if product_id in {'id-prod-navegante-' + city for city in MUNICIPALITIES} or 'municipal' in normalized(name):
        return 'municipal'
    return None

def purchase_block(card, product_id, details, variant_id, today=None):
    today = today or date.today()
    attribute = next((a for a in details.get('attributes', []) if a.get('id') == 'product_monthly'), None)
    if not attribute:
        return None  # Zapping, complements and other products keep their official rules.
    option = next((v for v in attribute.get('values', []) if v.get('id') == variant_id), None)
    if not option:
        return None
    value = str(option.get('value', ''))
    if re.fullmatch(r'\d{1,2}', value) and 1 <= int(value) <= 12:
        month = int(value)
        # Month-only API values refer to the closest occurrence, including Dec/Jan.
        year = min(range(today.year-1, today.year+2), key=lambda y: abs((y-today.year)*12+month-today.month))
    elif re.fullmatch(r'\d{4}-\d{2}(?:-\d{2})?', value):
        year, month = map(int, value[:7].split('-'))
    else:
        return None
    wanted = monthly_kind(product_id, details.get('name'))
    for item in (card or {}).get('contracts', []):
        fields = {f.get('id'): f for f in item.get('contract', [])}
        def raw(key):
            f = fields.get('contract.'+key, {})
            return str(f.get('value_id') if f.get('value_id') is not None else f.get('value', ''))
        if raw('temporal_validity.status').upper() in ('3', 'EXPIRED'):
            continue
        product = (item.get('product_detail') or {}).get('product') or {}
        existing_name = fields.get('contract.name', {}).get('value') or product.get('name', '')
        existing_id = product.get('id') or raw('product_id') or fields.get('contract.name', {}).get('value_id', '')
        existing = monthly_kind(existing_id, existing_name)
        if raw('temporal_validity.month').isdigit() and re.fullmatch(r'\d{4}(?:-\d{2}-\d{2})?', raw('temporal_validity.year')):
            period = (int(raw('temporal_validity.year')[:4]), int(raw('temporal_validity.month')))
        elif existing and re.fullmatch(r'\d{4}-\d{2}-\d{2}', raw('temporal_validity.date')):
            # Monthly cards may provide only their expiry date, not month/year.
            try:
                expiry = date.fromisoformat(raw('temporal_validity.date'))
            except ValueError:
                continue
            period = (expiry.year, expiry.month)
        else:
            continue  # No guessed duplicate when the existing period is unknown.
        if period != (year, month):
            continue
        same = bool(existing_id and existing_id == product_id) or bool(normalized(existing_name) and normalized(existing_name) == normalized(details.get('name')))
        if same or (existing == 'metro' and wanted in ('metro', 'municipal')):
            return ('Já tens um passe Metropolitano carregado para este período; não precisas de outro passe municipal ou metropolitano.'
                    if existing == 'metro' else 'Este passe já está carregado para o período escolhido.')
    return None

def annotate_offers(card, product_id, details):
    return {**details, 'prices': [{**price, 'blockedReason': purchase_block(card, product_id, details, price.get('id'))}
                                for price in details.get('prices', [])]}
