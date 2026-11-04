"""Application operations. The HTTP layer accepts choices, never card commands."""
import re
import time
import official
from commerce import identifier, email_address, nif, prepare
from purchase_rules import annotate_offers
from pcsc import PCSC
from sessions import Sessions
from tiptop import IntegrationError, read_card, catalog, product, load_card

class ServiceError(IntegrationError):
    def __init__(self, message, status=400):
        super().__init__(message)
        self.status = status

class Service:
    def __init__(self):
        self.sessions = Sessions()

    def session(self, data):
        session = self.sessions.get(data.get('session'))
        if not session:
            raise ServiceError('A leitura expirou. Lê novamente o cartão.', 410)
        return session

    def run(self, operation, data):
        if operation == 'recover':
            sale_id = identifier(data.get('saleId'))
            status = official.sale_status(sale_id, None)
            if not isinstance(status, dict):
                raise ServiceError('A referência não foi reconhecida pelo serviço oficial.')
            token = self.sessions.create({'reader': data.get('reader'), 'purchase': {'saleId': sale_id, 'state': 'pending'},
                                          'recovery': True})
            return {'session': token, 'saleId': sale_id}
        if operation in ('inspect', 'read'):
            reader = data.get('reader')
            if not isinstance(reader, str):
                raise ServiceError('Seleciona um leitor.')
            if operation == 'inspect':
                return PCSC().inspect(reader)
            with PCSC().connect(reader) as transmit:
                result = read_card(transmit)
            token = self.sessions.create({'reader': reader, **result})
            return {'session': token, 'card': result['result'].get('data'), 'expiresIn': self.sessions.ttl}
        session = self.session(data)
        if operation in ('catalog', 'product', 'prepare'):
            if session.get('purchase'):
                raise ServiceError('Conclui o pedido existente antes de escolher outro produto.')
            if 'catalog' not in session:
                session['catalog'] = catalog(session['read_id'])
            if operation == 'catalog':
                return session['catalog']
            pid = identifier(data.get('product'))
            if operation == 'product':
                details = product(session['read_id'], pid, session['catalog'])
                session.setdefault('products', {})[pid] = details
                session.pop('quote', None)
                return annotate_offers((session.get('result') or {}).get('data'), pid, details)
            return prepare(session, pid, data.get('variant'))
        if operation == 'checkout':
            payment_token = None
            quote = session.get('quote')
            if not quote or time.monotonic() - quote['created'] > 300:
                raise ServiceError('Consulta novamente o preço antes de pagar.')
            if session.get('purchase'):
                raise ServiceError('Já existe um pedido. Consulta o estado ou o histórico, sem repetir o pagamento.', 409)
            if type(data.get('confirmedAmount')) is not int or data['confirmedAmount'] != quote['amount']:
                raise ServiceError('Confirma o valor atualizado antes de pagar.')
            phone = data.get('phone', '')
            if quote['amount'] and (not isinstance(phone, str) or not re.fullmatch(r'9\d{8}', phone)):
                raise ServiceError('Indica um número português com nove dígitos para MB WAY.')
            email, fiscal = email_address(data.get('email')), nif(data.get('nif'))
            session_id = identifier((session['result'].get('data') or {}).get('sessionId'))
            session['purchase'] = {'state': 'submitting'}
            session['expires'] = time.monotonic() + 7200
            try:
                sale = official.checkout(session_id, email, phone, fiscal, payment_token, free=quote['amount'] == 0)
                if not isinstance(sale, dict):
                    raise IntegrationError('Resposta do pedido inválida. Consulta a referência do pedido antes de repetir.')
                sale_id = identifier(sale.get('saleId'))
                # Keep the reference even if other response fields are unexpected.
                session['purchase'].update(saleId=sale_id, state='pending')
                payment = sale.get('payment')
                amount = payment.get('amount') if isinstance(payment, dict) else 0
                if type(amount) is not int or amount != quote['amount']:
                    session['purchase']['state'] = 'amount_mismatch'
                    raise IntegrationError('O valor do pedido diverge do preço confirmado. Não aproves o pagamento; consulta a referência do pedido.')
                return {'saleId': sale_id, 'amount': amount, 'state': 'pending'}
            except Exception:
                if session['purchase']['state'] == 'submitting':
                    session['purchase']['state'] = 'unknown'
                raise
        if operation in ('payment', 'load'):
            payment_token = None
            purchase = session.get('purchase') or {}
            sale_id = purchase.get('saleId')
            if not sale_id:
                raise ServiceError('Consulta a referência do pedido para recuperar a referência do pedido.', 409)
            status = official.sale_status(sale_id, payment_token)
            if not isinstance(status, dict):
                raise IntegrationError('Resposta de estado inválida.')
            if status.get('isLoaded') is True:
                purchase['state'] = 'loaded'
                return {'state': 'loaded', 'saleId': sale_id}
            if purchase.get('state') == 'uncertain' and operation == 'load':
                raise ServiceError('A gravação anterior ficou por confirmar. Atualiza o estado e consulta o apoio Navegante antes de repetir.', 409)
            confirmation = official.payment_status(sale_id, payment_token)
            raw = str(confirmation.get('status', '')).lower() if isinstance(confirmation, dict) else ''
            state = {'paid': 'paid', 'created': 'pending', 'pending_payment': 'pending',
                     'canceled': 'canceled', 'refunded': 'refunded', 'pending_refund': 'refund_pending'}.get(raw, 'unknown')
            if state == 'paid' and isinstance(confirmation.get('loadRequest'), dict):
                session['load_request'] = confirmation['loadRequest']
            if purchase.get('state') != 'uncertain':
                purchase['state'] = state
            if operation == 'payment':
                return {'state': purchase['state'], 'saleId': sale_id, 'canLoad': state == 'paid' and bool(session.get('load_request')) and purchase['state'] != 'uncertain'}
            if state != 'paid' or not session.get('load_request'):
                raise ServiceError('O serviço ainda não autorizou o carregamento deste pedido.', 409)
            reader = data.get('reader') or session.get('reader')
            if not isinstance(reader, str):
                raise ServiceError('Seleciona o leitor com o passe que foi comprado.')
            if data.get('confirmWrite') is not True:
                raise ServiceError('Confirma a gravação do título no cartão.')
            purchase['state'] = 'loading'
            write_started = False
            def progress(command):
                nonlocal write_started
                if command[1] not in (0xA4, 0xB0, 0xB2, 0xC0, 0xCA, 0x8A, 0x20, 0x84, 0x7C):
                    write_started = True
            try:
                with PCSC().connect(reader) as transmit:
                    load_card(transmit, session['load_request'], progress)
                verified = official.sale_status(sale_id, payment_token)
                if not isinstance(verified, dict) or verified.get('isLoaded') is not True:
                    purchase['state'] = 'uncertain'
                    raise IntegrationError('O servidor ainda não confirmou a gravação. Atualiza o estado sem repetir o pagamento.')
                purchase['state'] = 'loaded'
                session.pop('load_request', None)
                return {'state': 'loaded', 'saleId': sale_id}
            except Exception:
                purchase['state'] = 'uncertain' if write_started else 'paid'
                raise
        raise ServiceError('Operação não encontrada.', 404)
