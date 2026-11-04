"""TipTop read protocol recovered from the user-supplied Android application.

Only read operations are enabled. Credentials and card data are never written to disk.
"""
import base64
import json
import re
from urllib.request import Request, urlopen
from urllib.error import HTTPError, URLError

URL = 'https://lisbon.tiptop.spiws.net/tiptop/'
HEADERS = {'Accept': 'application/json', 'Content-Type': 'application/json'}

class IntegrationError(Exception):
    pass

def gateway(method, path, body=None):
    request = {'method': method, 'path': path, 'headers': HEADERS}
    if body is not None:
        request['body'] = json.dumps(body, separators=(',', ':'))
    return exchange({'id': 201, 'version': 1, 'request': request})

def exchange(payload):
    data = json.dumps(payload).encode()
    try:
        with urlopen(Request(URL, data=data, headers=HEADERS), timeout=25) as response:
            raw = response.read(2_000_001)
        if len(raw) > 2_000_000:
            raise IntegrationError('Resposta do serviço demasiado grande.')
        envelope = json.loads(raw)
        if 'error' in envelope:
            raise IntegrationError(f'O serviço de bilhética devolveu o erro {envelope["error"]}.')
        status = envelope.get('status', 0)
        if not isinstance(status, int) or not 200 <= status < 300:
            raise IntegrationError(f'O serviço de bilhética devolveu o estado {status}.')
        result = envelope.get('body')
        return json.loads(result) if isinstance(result, str) and result else (None if result == '' else result)
    except (HTTPError, URLError, TimeoutError) as exc:
        raise IntegrationError('Não foi possível contactar o serviço de bilhética.') from exc
    except (ValueError, TypeError) as exc:
        raise IntegrationError('Resposta inválida do serviço de bilhética.') from exc

def read_only_apdu(command):
    """Fail closed: read/authentication only, no update, debit or PIN instruction."""
    if not 4 <= len(command) <= 261 or command[0] not in (0x00, 0x94):
        return False
    ins = command[1]
    if ins == 0xA4:
        return len(command) >= 5 and len(command) in (5 + command[4], 6 + command[4])
    if ins in (0x8A, 0x8E):
        # Open/close an authenticated Calypso read session. No mutation commands
        # are permitted between these operations.
        return len(command) >= 5 and command[4] in (4, 8) and len(command) in (5 + command[4], 6 + command[4])
    if ins == 0x20:
        # The official read service supplies an encrypted four-byte Calypso PIN
        # after opening its authenticated session. No guessed/user PINs.
        return len(command) == 9 and command[2:5] == b'\0\0\x04'
    return ins in (0xB0, 0xB2, 0xC0, 0xCA) and len(command) == 5

def execute_read_commands(encoded, transmit, *, loading=False):
    try:
        commands = base64.b64decode(encoded, validate=True)
    except (ValueError, TypeError) as exc:
        raise IntegrationError('Comandos de leitura inválidos.') from exc
    if len(commands) < 2 or commands[0] != 2 or len(commands) > 65536:
        raise IntegrationError('Versão ou tamanho de comandos não suportados.')
    offset, count, answers = 2, 0, []
    parsed = []
    # Validate the entire batch before sending any command to the card.
    while offset < len(commands):
        if offset + 2 > len(commands):
            raise IntegrationError('Comando incompleto.')
        opcode, flags = commands[offset:offset + 2]
        if opcode == 73:
            if offset + 6 > len(commands):
                raise IntegrationError('Comando APDU incompleto.')
            length = int.from_bytes(commands[offset + 2:offset + 6], 'big')
            end = offset + 6 + length
            apdu = commands[offset + 6:end]
            allowed = read_only_apdu(apdu) or (loading and load_apdu(apdu))
            if end > len(commands) or not allowed:
                raise IntegrationError('O servidor pediu um comando fora do modo de leitura suportado.')
        elif opcode == 121:
            end, apdu = offset + 6, None
        elif opcode == 122:
            end, apdu = offset + 2, None
        else:
            raise IntegrationError(f'Ação de leitura não suportada: {opcode}.')
        if end > len(commands):
            raise IntegrationError('Comando incompleto.')
        parsed.append((opcode, flags, apdu))
        offset = end
    if len(parsed) != commands[1]:
        raise IntegrationError('Contagem de comandos inválida.')
    for opcode, flags, apdu in parsed:
        count += 1
        answer = list(transmit(apdu)) if opcode == 73 else ([0, 0, 0, 8, 0] if opcode == 121 else [])
        answers.extend([opcode, 0, 0, 0, 0])
        if opcode == 73:
            answers.extend(len(answer).to_bytes(4, 'big'))
        answers.extend(answer)
    return [2, count, *answers]

def read_card(transmit, call=gateway):
    opened, pin_used = False, False
    def channel(command):
        nonlocal opened, pin_used
        if command[1] == 0x20:
            if not opened or pin_used:
                raise IntegrationError('Verificação de acesso fora da sessão de leitura.')
            pin_used = True
        response = transmit(command)
        if command[1] == 0x8A:
            opened = response[-2:] == b'\x90\x00'
            if not opened:
                raise IntegrationError('Não foi possível autenticar a sessão de leitura do cartão.')
        if command[1] == 0x20 and response[-2:] != b'\x90\x00':
            raise IntegrationError('O cartão recusou a verificação de acesso. Não foi feita nova tentativa.')
        return response
    result = call('POST', 'reads?device=tiptop', [2, 1, 121, 0, 0, 0, 0, 0, 0, 0, 8, 0])
    read_id = None
    for _ in range(48):
        if not isinstance(result, dict):
            raise IntegrationError('Formato de leitura inesperado.')
        if result.get('id') is not None:
            candidate = str(result['id'])
            if not re.fullmatch(r'[A-Za-z0-9_-]{1,100}', candidate):
                raise IntegrationError('Identificador de leitura inválido.')
            if read_id and candidate != read_id:
                raise IntegrationError('O identificador da leitura mudou durante a operação.')
            read_id = candidate
        if not result.get('PO_command'):
            if read_id is None:
                raise IntegrationError('O serviço não forneceu um identificador de leitura.')
            if not isinstance(result.get('data'), dict):
                raise IntegrationError('O serviço terminou a leitura sem devolver os dados do cartão.')
            return {'read_id': read_id, 'result': result}
        if read_id is None:
            raise IntegrationError('O serviço não forneceu um identificador de leitura.')
        answers = execute_read_commands(result['PO_command'], channel)
        result = call('PUT', f'reads/{read_id}', answers)
    raise IntegrationError('A leitura excedeu o número de passos esperado.')

def catalog(read_id):
    if not re.fullmatch(r'[A-Za-z0-9_-]{1,100}', read_id):
        raise IntegrationError('Identificador inválido.')
    return gateway('GET', f'products?read_id={read_id}')

def product_ids(catalog_data):
    if not isinstance(catalog_data, dict):
        return set()
    ids = {p['id'] for p in catalog_data.get('products', []) if isinstance(p, dict) and isinstance(p.get('id'), str)}
    for group in catalog_data.get('groups', []):
        ids.update(product_ids(group))
    return ids

def product(read_id, product_id, catalog_data):
    if product_id not in product_ids(catalog_data) or not re.fullmatch(r'[A-Za-z0-9_-]{1,100}', product_id):
        raise IntegrationError('Produto não disponível para esta leitura.')
    return gateway('GET', f'products/{product_id}?read_id={read_id}')

def load_apdu(command):
    """Ticket writes only, reachable exclusively from an official signed load."""
    if not 5 <= len(command) <= 261 or command[0] not in (0x00, 0x94):
        return False
    if command[1] in (0x84, 0x7C):
        return len(command) == 5
    if command[1] not in (0xDC, 0xD2, 0xE2, 0xD6, 0x32, 0x30, 0x3A, 0x38, 0xB8):
        return False
    return command[4] > 0 and len(command) in (5 + command[4], 6 + command[4])

def load_card(transmit, signed, progress=None, call=gateway, signed_call=exchange):
    if not isinstance(signed, dict) or not all(isinstance(signed.get(k), str) and signed[k] for k in ('request64', 'signature', 'keyRef')):
        raise IntegrationError('O serviço não forneceu autorização de carregamento.')
    try:
        original = json.loads(base64.b64decode(signed['request64'], validate=True))
        path = original['path']
        if original['method'] != 'POST' or not re.fullmatch(r'loads(?:\?[A-Za-z0-9_=&%.-]*)?', path):
            raise ValueError()
    except (ValueError, KeyError, TypeError) as exc:
        raise IntegrationError('Autorização de carregamento com formato não suportado.') from exc
    result = signed_call({'id': 201, 'version': 1, **{k: signed[k] for k in ('request64', 'signature', 'keyRef')}})
    identifier = None
    selected = False
    for _ in range(64):
        if not isinstance(result, dict):
            raise IntegrationError('Resposta de carregamento inválida.')
        if result.get('id') is not None:
            candidate = str(result['id'])
            if not re.fullmatch(r'[A-Za-z0-9_-]{1,100}', candidate) or identifier and candidate != identifier:
                raise IntegrationError('Identificador de carregamento inválido.')
            identifier = candidate
        if not result.get('PO_command'):
            return result
        if identifier is None:
            raise IntegrationError('Carregamento sem identificador.')
        def channel(command):
            nonlocal selected
            if not selected and command[1] != 0xA4:
                aid = bytes.fromhex('315449432E494341')
                response = transmit(bytes.fromhex('00A40400') + bytes([len(aid)]) + aid)
                if response[-2:] != b'\x90\0':
                    raise IntegrationError('Não foi possível selecionar a aplicação do passe.')
            selected = True
            if progress:
                progress(command)
            return transmit(command)
        answers = execute_read_commands(result['PO_command'], channel, loading=True)
        result = call('PUT', 'loads/' + identifier, answers)
    raise IntegrationError('O carregamento excedeu o número de passos esperado.')
