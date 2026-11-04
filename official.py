"""Guest payments API. No user-account authentication is required by this client."""
import json
from urllib.request import Request, urlopen
from urllib.error import HTTPError, URLError
from tiptop import IntegrationError

EDGE = 'https://pcgi.tmlmobilidade.pt/app/edge/'
MY = 'https://pcgi.tmlmobilidade.pt/my-navegante/api/'
# The service rejects generic clients before authentication. This identifies the
# compatible app protocol version and explicitly identifies this PC/SC client.
USER_AGENT = 'navegante/2026.10.19 android;NaveganteWeb PCSC interoperability'

class AccountError(IntegrationError):
    def __init__(self, message, status=502):
        super().__init__(message)
        self.status = status

def request(method, base, path, body=None, token=None):
    headers = {'Accept': 'application/json', 'Content-Type': 'application/json', 'User-Agent': USER_AGENT}
    if base == MY:
        headers.update({'Origin': 'https://my.navegante.pt', 'Referer': 'https://my.navegante.pt/'})
    if token:
        headers['Authorization'] = 'Bearer ' + token
    data = json.dumps(body).encode() if body is not None else (b'' if method == 'POST' else None)
    try:
        with urlopen(Request(base + path, data=data, headers=headers, method=method), timeout=30) as response:
            raw = response.read(2_000_001)
        if len(raw) > 2_000_000:
            raise AccountError('Resposta do serviço demasiado grande.')
        return json.loads(raw) if raw else None
    except HTTPError as exc:
        messages = {400: 'O serviço recusou os dados enviados.', 401: 'O serviço oficial recusou a autorização deste pedido de compra.',
                    403: 'O serviço não autorizou esta operação.', 409: 'Existe uma operação em conflito. Consulta a referência do pedido.',
                    422: 'Os dados não são válidos para esta operação.', 429: 'Demasiadas tentativas. Aguarda antes de tentar novamente.',
                    503: 'O serviço Navegante respondeu 503 (indisponível). Tenta consultar o estado do pedido mais tarde.'}
        raise AccountError(messages.get(exc.code, f'O serviço oficial devolveu HTTP {exc.code}.'), exc.code) from exc
    except (URLError, TimeoutError, OSError) as exc:
        raise AccountError('A ligação ao serviço falhou. Se iniciaste um pagamento, consulta a referência do pedido antes de repetir.') from exc
    except (ValueError, TypeError) as exc:
        raise AccountError('Resposta inválida do serviço oficial.') from exc

def checkout(session_id, email, phone, fiscal_number, token=None, free=False):
    body = {'customer': {'name': None, 'email': email, 'phone': '351#' + phone if phone else None,
                         'fiscalNumber': fiscal_number or None, 'address': None},
            'paymentMethod': None if free else 'MBWAY'}
    return request('POST', EDGE, 'payments/checkout/' + session_id, body, token)

def payment_status(sale_id, token=None):
    return request('POST', EDGE, 'payments/confirmation/' + sale_id, token=token)

def sale_status(sale_id, token=None):
    return request('GET', EDGE, 'sales/' + sale_id + '/status', token=token)

def history(token):
    return request('GET', EDGE, 'sales/history', token=token)
