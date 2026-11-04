"""Collect declared licenses and bundled notices for the Windows Cargo graph.

This preserves upstream texts; it is an inventory, not legal/provider approval.
It does not execute dependency build scripts or contact transport services.
"""
import argparse
import json
from pathlib import Path
import shutil
import subprocess

ROOT = Path(__file__).resolve().parents[1]


def collect(target):
    cargo = shutil.which('cargo') or str(Path.home() / '.cargo/bin/cargo.exe')
    raw = subprocess.check_output([
        cargo, 'metadata', '--locked', '--manifest-path', str(ROOT / 'native/Cargo.toml'),
        '--format-version', '1', '--filter-platform', target,
    ])
    metadata = json.loads(raw)
    overrides = json.loads((ROOT / 'native/licenses/upstream.json').read_text(encoding='utf-8'))
    active = {node['id'] for node in metadata['resolve']['nodes']}
    report, sections = [], []
    for package in sorted(metadata['packages'], key=lambda p: (p['name'], p['version'])):
        if package['id'] not in active or package['name'] == 'navegante-desktop':
            continue
        if not package.get('license') and not package.get('license_file'):
            raise RuntimeError(f"Missing license declaration: {package['name']}")
        directory = Path(package['manifest_path']).parent
        files = set()
        for file in directory.rglob('*'):
            if not file.is_file():
                continue
            name = file.name.lower()
            is_notice = any(word in name for word in ('license', 'licence', 'copyright', 'copying', 'notice'))
            is_font_notice = file.parent.name == 'fonts' and file.suffix.lower() == '.txt'
            if is_notice or is_font_notice:
                files.add(file)
        if package.get('license_file'):
            files.add(directory / package['license_file'])
        texts = []
        for file in sorted(files):
            if file.stat().st_size > 2_000_000:
                raise RuntimeError(f'Oversized notice: {package["name"]}/{file.name}')
            texts.append(f'--- {file.relative_to(directory).as_posix()} ---\n' + file.read_text(encoding='utf-8-sig'))
        upstream = []
        if not texts and package['name'] in overrides:
            vcs = json.loads((directory / '.cargo_vcs_info.json').read_text(encoding='utf-8'))['git']['sha1']
            for notice in overrides[package['name']]:
                if '/' + vcs + '/' not in notice['source']:
                    raise RuntimeError(f'Update upstream notices for new version of {package["name"]}')
                file = ROOT / 'native' / notice['file']
                texts.append(f'--- Upstream: {notice["source"]} ---\n' + file.read_text(encoding='utf-8-sig'))
                upstream.append(notice)
        if not texts:
            raise RuntimeError(f'No bundled or pinned upstream license texts: {package["name"]}')
        record = {key: package.get(key) for key in ('name', 'version', 'license', 'repository', 'authors')}
        record['bundled_notice_files'] = [file.relative_to(directory).as_posix() for file in sorted(files)]
        record['pinned_upstream_notices'] = upstream
        report.append(record)
        sections.append('\n'.join([
            '=' * 72, f"{package['name']} {package['version']}",
            'Declared license: ' + str(package.get('license') or 'see bundled license'),
            'Authors: ' + ', '.join(package.get('authors') or []),
            'Source: ' + str(package.get('repository') or 'https://crates.io/crates/' + package['name']),
            *texts,
        ]))
    return report, sections


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--target', default='x86_64-pc-windows-msvc')
    parser.add_argument('--output', type=Path, default=ROOT / 'release')
    args = parser.parse_args()
    report, sections = collect(args.target)
    for file in sorted((ROOT / 'native/assets/fonts').glob('*-OFL.txt')):
        sections.append('=' * 72 + '\nBundled UI font: ' + file.name + '\n' + file.read_text(encoding='utf-8'))
    args.output.mkdir(parents=True, exist_ok=True)
    heading = ('Third-party dependency notices for Navegante Desktop\n'
               'Original project code is MIT licensed; dependencies retain their own licenses.\n'
               'Inventory includes the resolved Windows graph and build dependencies.\n'
               'Declared expressions and upstream bundled texts follow. No legal approval is implied.\n\n')
    (args.output / 'THIRD-PARTY-NOTICES.txt').write_text(heading + '\n\n'.join(sections), encoding='utf-8')
    (args.output / 'dependency-licenses.json').write_text(json.dumps(report, ensure_ascii=False, indent=2) + '\n', encoding='utf-8')
    print(f'Collected {len(report)} third-party declarations and their bundled license/notice texts.')
    print('License texts are present for every dependency. Provider eligibility review remains separate.')


if __name__ == '__main__':
    main()
