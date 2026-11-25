#!/usr/bin/env python3
"""Compile .xcstrings catalogs into .lproj/*.strings + *.stringsdict without Xcode."""
import json, plistlib, re, sys
from pathlib import Path

SPEC = re.compile(r'%(?:\d+\$)?[-+ #0]*\d*(?:\.\d+)?(?:hh|h|ll|l|q|z|t|j)?[@dDuUxXoOfeEgGcCsSpaA]')

import os
BRAND = os.environ.get('MACN_BRAND')

def brand(table, key, value):
    """MACN-cheese: show the port's name instead of iPadn in user-facing text."""
    if not BRAND: return value
    if table == 'InfoPlist' and key in ('CFBundleDisplayName', 'CFBundleName'): return BRAND
    return value.replace('iPadn', BRAND)

# Keys whose visible text becomes "Download" on the Mac (value copied from the
# key on the right, so every language gets its own word for Download).
MAC_DOWNLOAD = {'安装': '下载', '一键安装': '下载', '安装新版本': '下载'}
MAC_DOWNLOAD_EN = {'安装 %@': 'Download %@', '下载与安装': 'Downloads', '安装与导出': 'Download & Export',
                   '一键安装（推荐）': 'Download (recommended)', '安装新版本': 'Download New Version'}

def esc(s):
    return s.replace('\\', '\\\\').replace('"', '\\"').replace('\n', '\\n').replace('\t', '\\t')

def unit(node):
    su = node.get('stringUnit') if isinstance(node, dict) else None
    return su.get('value') if su else None

def compile_catalog(path, outdir, table):
    cat = json.loads(Path(path).read_text(encoding='utf-8'))
    src = cat.get('sourceLanguage', 'en')
    langs = set()
    for entry in cat['strings'].values():
        langs.update(entry.get('localizations', {}).keys())
    langs.add(src)
    for lang in sorted(langs):
        strings, plurals = {}, {}
        for key, entry in cat['strings'].items():
            loc = entry.get('localizations', {}).get(lang)
            if loc is None:
                if lang == src:
                    strings[key] = key
                continue
            v = unit(loc)
            if v is not None:
                strings[key] = v; continue
            plural = loc.get('variations', {}).get('plural')
            if plural:
                forms = {k: unit(n) for k, n in plural.items() if unit(n) is not None}
                if 'other' in forms:
                    m = SPEC.search(forms['other'])
                    vt = m.group(0)[1:].split('$')[-1].lstrip('-+ #0123456789.') if m else 'lld'
                    rule = {'NSStringFormatSpecTypeKey': 'NSStringPluralRuleType', 'NSStringFormatValueTypeKey': vt}
                    for k, val in forms.items():
                        rule[k] = val
                    plurals[key] = {'NSStringLocalizedFormatKey': '%#@v@', 'v': rule}
                    strings[key] = forms['other']
                continue
            dev = loc.get('variations', {}).get('device')
            if dev:
                for pref in ('mac', 'ipad', 'other', 'iphone'):
                    if pref in dev and unit(dev[pref]) is not None:
                        strings[key] = unit(dev[pref]); break
            if key not in strings and lang == src:
                strings[key] = key
        d = Path(outdir) / f'{lang}.lproj'
        d.mkdir(parents=True, exist_ok=True)
        strings = {k: brand(table, k, v) for k, v in strings.items()}
        if BRAND and table == 'Localizable':
            # MACN-cheese never installs anything: "Install" buttons download instead.
            for k, src_key in MAC_DOWNLOAD.items():
                if k in strings and src_key in strings: strings[k] = strings[src_key]
            if lang == 'en':
                strings.update({k: v for k, v in MAC_DOWNLOAD_EN.items() if k in strings})
        if table == 'InfoPlist' and BRAND:
            strings.setdefault('CFBundleDisplayName', BRAND); strings.setdefault('CFBundleName', BRAND)
        for rule in plurals.values():
            for f, v in rule['v'].items():
                if f not in ('NSStringFormatSpecTypeKey', 'NSStringFormatValueTypeKey'): rule['v'][f] = brand(table, '', v)
        (d / f'{table}.strings').write_text(''.join(f'"{esc(k)}" = "{esc(v)}";\n' for k, v in strings.items()), encoding='utf-8')
        if plurals:
            (d / f'{table}.stringsdict').write_bytes(plistlib.dumps(plurals))

if __name__ == '__main__':
    outdir = sys.argv[1]
    for p in sys.argv[2:]:
        compile_catalog(p, outdir, Path(p).stem)
