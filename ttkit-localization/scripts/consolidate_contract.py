import re
import json
from pathlib import Path

script_dir = Path(__file__).resolve().parent.parent
catalogs_dir = script_dir.parent / "core/rust/ttzip-engine/src/i18n/catalogs"
if not catalogs_dir.exists():
    catalogs_dir = script_dir / "tt-i18n-core/src/catalogs"
langs = {
    "en": "en.rs",
    "zh-Hans": "zh_hans.rs",
    "zh-Hant": "zh_hant.rs",
    "ja": "ja.rs",
    "de": "de.rs",
    "fr": "fr.rs",
    "es": "es.rs",
}

translations_by_lang = {}
pattern = re.compile(r'\(\s*"([^"]+)"\s*,\s*"((?:[^"\\]|\\.)*)"\s*\)')

for lang, filename in langs.items():
    filepath = catalogs_dir / filename
    content = filepath.read_text(encoding='utf-8')
    trans_map = {}
    for match in pattern.finditer(content):
        key = match.group(1)
        val = match.group(2).replace('\\"', '"').replace('\\\\', '\\')
        trans_map[key] = val
    translations_by_lang[lang] = trans_map
    print(f"Loaded {len(trans_map)} keys for {lang}")

# Consolidate into canonical entries
all_keys = sorted(translations_by_lang["en"].keys())
entries = {}

for k in all_keys:
    translations = {}
    for lang in langs:
        translations[lang] = translations_by_lang[lang].get(k, translations_by_lang["en"][k])
    
    entries[k] = {
        "key": k,
        "description": f"Localization entry for {k}",
        "translations": translations,
        "placeholders": re.findall(r'%(\d+\$)?[@sdf]', translations_by_lang["en"][k])
    }

contract = {
    "$schema": "http://json-schema.org/draft-07/schema#",
    "namespace": "ttzip",
    "entries": entries
}

out_path = script_dir / "contracts/ttzip-catalog.json"
with open(out_path, "w", encoding="utf-8") as f:
    json.dump(contract, f, ensure_ascii=False, indent=2)

print(f"Successfully generated {out_path} with {len(entries)} consolidated entries.")
