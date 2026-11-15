"""Checks AnyFont's translation files against the English master list.

    python -I scripts/i18n_check.py [<repo>] [<code> ...]

Reads the keys of docs/strings-en.tsv (one English string per line, `#` lines are comments) and checks
crates/anyfont/assets/i18n/<code>.tsv (one `English<TAB>translation` pair per line) for every language
AnyFont ships, or only the codes given. It reports:

- keys of strings-en.tsv that are missing from the translation file
- keys in the translation file that strings-en.tsv does not have (extra keys)
- keys that are in the file twice, lines without a tab, empty translations
- translations whose `{}` / `{0}` placeholder count differs from the English, or whose `{n}` index is out of range
- translations whose `\\n` line-break count differs from the English
- translations with more than one `&` access key (`&&` is a literal ampersand and is not counted)

Exits with 1 when something is wrong.
"""
import os
import re
import sys

LANGS = ['pt-BR', 'zh-CN', 'zh-TW', 'cs-CZ', 'nl-NL', 'fr-FR', 'de-DE', 'it-IT', 'ja-JP', 'pl-PL',
         'ru-RU', 'es-ES', 'sv-SE', 'tr-TR']

args = sys.argv[1:]
if args and os.path.isdir(args[0]):
    root = args.pop(0)
else:
    root = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
langs = args or LANGS

master = os.path.join(root, 'docs', 'strings-en.tsv')
assets = os.path.join(root, 'crates', 'anyfont', 'assets', 'i18n')

PH = re.compile(r'\{(\d*)\}')


def master_keys():
    keys, seen = [], set()
    with open(master, encoding='utf-8-sig') as f:
        for row in f:
            row = row.rstrip('\n').rstrip('\r')
            if not row or row.startswith('#'):
                continue
            if row not in seen:
                seen.add(row)
                keys.append(row)
    return keys


def amp_count(s):
    """Single `&` characters (access-key markers); `&&` is a literal ampersand."""
    return s.replace('&&', '').count('&')


def check(lang, keys):
    path = os.path.join(assets, f'{lang}.tsv')
    if not os.path.exists(path):
        return [f'file not found: {path}'], 0
    problems, d = [], {}
    with open(path, encoding='utf-8-sig') as f:
        for n, row in enumerate(f, 1):
            row = row.rstrip('\n').rstrip('\r')
            if not row or row.startswith('#'):
                continue
            if '\t' not in row:
                problems.append(f'line {n}: no tab: {row}')
                continue
            k, v = row.split('\t', 1)
            if k in d:
                problems.append(f'line {n}: duplicate key: {k}')
            d[k] = v
            if not v.strip():
                problems.append(f'line {n}: empty translation: {k}')
                continue
            if '\t' in v:
                problems.append(f'line {n}: tab in the translation: {k}')
            kp, vp = PH.findall(k), PH.findall(v)
            if len(kp) != len(vp):
                problems.append(f'line {n}: placeholders {len(kp)} -> {len(vp)}: {k}  ->  {v}')
            elif any(i and int(i) >= len(kp) for i in vp):
                problems.append(f'line {n}: placeholder index out of range: {v}')
            elif any(vp) and not all(vp):
                problems.append(f'line {n}: mixes {{}} and {{n}} placeholders: {v}')
            if k.count('\\n') != v.count('\\n'):
                problems.append(f'line {n}: line breaks differ: {k}  ->  {v}')
            if amp_count(v) > 1:
                problems.append(f'line {n}: more than one access key: {v}')
    keyset = set(keys)
    for k in keys:
        if k not in d:
            problems.append(f'missing: {k}')
    for k in d:
        if k not in keyset:
            problems.append(f'extra key: {k}')
    return problems, len(d)


keys = master_keys()
failed = False
for lang in langs:
    problems, have = check(lang, keys)
    print(f'{lang}: {len(keys)} keys, {have} lines, {len(problems)} problems')
    for p in problems[:40]:
        print('  ', p)
    if len(problems) > 40:
        print(f'   ... and {len(problems) - 40} more')
    failed |= bool(problems)
print('FAILED' if failed else 'OK')
sys.exit(1 if failed else 0)
