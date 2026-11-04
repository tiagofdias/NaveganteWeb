import http.client
from pathlib import Path
from unittest import TestCase
from unittest.mock import patch
import desktop

class DesktopTests(TestCase):
    def test_desktop_launches_exact_local_page_in_own_window(self):
        captured = {}
        case = self
        class Child:
            def wait(self):
                url = next(arg for arg in captured['args'] if arg.startswith('--app=')).removeprefix('--app=')
                port = int(url.rsplit(':',1)[1])
                captured['port'] = port
                case.assertNotEqual(port,8765,'Desktop uses its own free port')
                connection = http.client.HTTPConnection('127.0.0.1',port,timeout=5)
                connection.request('GET','/')
                response = connection.getresponse()
                case.assertEqual(response.status,200)
                case.assertEqual(response.read(),(desktop.server.ROOT/'index.html').read_bytes())
                connection.close()
        def launch(args, **kwargs):
            captured['args'] = args
            return Child()
        with patch('desktop.edge_path',return_value=Path('msedge.exe')),patch('desktop.subprocess.Popen',side_effect=launch):
            desktop.main()
        case.assertTrue(any(arg.startswith('--user-data-dir=') for arg in captured['args']))
        case.assertIn('--disable-background-mode',captured['args'])
        connection = http.client.HTTPConnection('127.0.0.1',captured['port'],timeout=1)
        with case.assertRaises(OSError):
            connection.request('GET','/')
        connection.close()
