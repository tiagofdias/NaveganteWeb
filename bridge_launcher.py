"""Interactive launcher for anyone using the hosted site with a local reader."""
import runpy
import sys
from bridge import BridgeAccess

if __name__ == '__main__':
    print('Navegante Web - ligar o site ao leitor deste PC')
    print('Copia o endereço HTTPS do site, sem caminhos adicionais.')
    try:
        origin = input('Endereço do site: ').strip()
        BridgeAccess(origin)
        if not origin:
            raise ValueError('Indica o endereço do site para autorizar a ligação.')
        sys.argv = ['server.py', '--web-origin', origin]
        runpy.run_module('server', run_name='__main__')
    except (KeyboardInterrupt, EOFError):
        print('\nPonte terminada.')
    except (ValueError, OSError) as exc:
        print(f'Não foi possível iniciar: {exc}')
        print('Se a porta 8765 estiver ocupada, fecha a outra instância de server.py.')
        sys.exit(1)
