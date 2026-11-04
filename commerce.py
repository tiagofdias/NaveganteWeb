"""Validated purchase state transitions; no payment is initiated during reads."""
import re
import time
from tiptop import IntegrationError, gateway
from purchase_rules import purchase_block

def identifier(value):
    if not isinstance(value, str) or not re.fullmatch(r'[A-Za-z0-9_-]{1,100}', value):
        raise IntegrationError('Identificador inválido.')
    return value

def email_address(value):
    if not isinstance(value, str) or len(value) > 254 or not re.fullmatch(r'[^\s@]+@[^\s@]+\.[^\s@]+', value):
        raise IntegrationError('Indica um endereço de e-mail válido.')
    return value.strip()

def nif(value):
    if value in (None, ''):
        return None
    if not isinstance(value, str) or not re.fullmatch(r'\d{9}', value):
        raise IntegrationError('O NIF deve ter nove dígitos.')
    total = sum(int(n) * (9 - i) for i, n in enumerate(value[:8]))
    expected = 11 - total % 11
    if int(value[-1]) != (0 if expected >= 10 else expected):
        raise IntegrationError('O NIF não é válido.')
    return value

def price_amount(price):
    try:
        # Official prices are integral euro cents. Never use float arithmetic.
        net, tax = str(price['net']), str(price['tax'])
        if not re.fullmatch(r'\d+', net) or not re.fullmatch(r'\d+', tax):
            raise ValueError()
        amount = int(net) + int(tax)
        if not 0 <= amount <= 100_000:
            raise ValueError()
        return amount
    except (ValueError, KeyError, TypeError) as exc:
        raise IntegrationError('Preço oficial inválido.') from exc

def prepare(session, product_id, variant_id, call=gateway):
    if session.get('purchase'):
        raise IntegrationError('Já existe um pedido nesta leitura. Consulta o pagamento ou o histórico.')
    details = session.get('products', {}).get(product_id)
    if not isinstance(details, dict):
        raise IntegrationError('Consulta primeiro as opções do produto.')
    prices = details.get('prices') or []
    selected = [p for p in prices if p.get('id') == variant_id]
    if len(selected) != 1:
        raise IntegrationError('Seleciona um preço devolvido pelo serviço oficial.')
    blocked = purchase_block((session.get('result') or {}).get('data'), product_id, details, variant_id)
    if blocked:
        raise IntegrationError(blocked)
    fields = []
    for attribute in details.get('attributes') or []:
        options = [v for v in attribute.get('values', []) if v.get('id') == variant_id]
        if len(options) != 1:
            raise IntegrationError('Esta configuração de produto ainda não é suportada.')
        # The app submits the selected value UUID as the field ID, rather than
        # the parent attribute name (e.g. product_monthly).
        fields.append({'id': options[0]['id'], 'value': options[0]['value']})
    price = selected[0]
    amount = price_amount(price)
    normalized_price = {k: price[k] for k in ('id', 'description') if price.get(k) is not None}
    for key in ('net', 'tax', 'tax_factor', 'quantity'):
        value = str(price.get(key, ''))
        if not re.fullmatch(r'\d+', value):
            raise IntegrationError('Parâmetros do preço oficial inválidos.')
        normalized_price[key] = int(value)
    payload = {'id': details['id'], 'attributes': fields,
               'prices': [normalized_price]}
    configured = call('PUT', f'products/{identifier(product_id)}?read_id={identifier(session["read_id"])}', payload)
    if isinstance(configured, dict):
        confirmed = [p for p in configured.get('prices', []) if p.get('id') == variant_id]
        if len(confirmed) != 1:
            raise IntegrationError('O serviço alterou as opções. Consulta novamente o produto.')
        amount = price_amount(confirmed[0])
        session['products'][product_id] = configured
    quote = {'product': product_id, 'variant': variant_id, 'name': details.get('name', product_id),
             'amount': amount, 'currency': 'EUR', 'created': time.monotonic()}
    session['quote'] = quote
    return {k: v for k, v in quote.items() if k != 'created'}
