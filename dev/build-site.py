#!/usr/bin/env python3
"""Build the guide from its tested Markdown sources and real Rust snapshots."""
import json
from pathlib import Path
import posixpath
import re
import shutil
import subprocess

ROOT = Path(__file__).resolve().parents[1]
STAGING = ROOT / 'target/site-src'
OUTPUT = ROOT / 'target/site-preview/how-far'
REPO = 'https://github.com/imazen/how-far/blob/main/'
PAGES = {
    'README.md': ('getting-started', 'Getting started'),
    'crates/how-far/README.md': ('library', 'Writing a library'),
    'crates/how-far-along/README.md': ('tracking', 'Tracking progress'),
    'crates/how-far-really/README.md': ('diagnostics', 'Diagnostics'),
    'docs/how-far-integration.md': ('integration', 'Stages and integration'),
    'docs/how-far-testing-and-tuning.md': ('testing', 'Testing and tuning'),
    'docs/how-far-design.md': ('design', 'Design and boundaries'),
    'docs/how-far-validation.md': ('validation', 'Validation and measurements'),
}

def content(source):
    text = (ROOT / source).read_text()
    text = re.sub(r'^# [^\n]*\n', '', text, count=1).lstrip()
    def link(match):
        label, url = match.groups()
        if url.startswith(REPO):
            url = url[len(REPO):]
            base = ''
        elif ':' in url or url.startswith(('#', '/')):
            return match.group(0)
        else:
            base = posixpath.dirname(source)
        path, sep, anchor = url.partition('#')
        path = posixpath.normpath(posixpath.join(base, path))
        if path in PAGES and anchor == 'forkjoin-with-different-operations':
            anchor = 'fork-join-with-different-operations'
        target = '/how-far/' + PAGES[path][0] + '/' if path in PAGES else REPO + path
        return f'[{label}]({target}{sep}{anchor})'
    text = re.sub(r'\[([^\]]*)\]\(([^\s)]+)\)', link, text)
    if '```mermaid' in text:
        # This guide has one fork/join diagram; reject new diagrams until rendered.
        diagrams = re.findall(r'```mermaid\n(.*?)```', text, re.S)
        expected = ['Previous stage', 'Pixel analysis', 'Read metadata', 'Join results', 'Next stage']
        if len(diagrams) != 1 or not all(label in diagrams[0] for label in expected):
            raise ValueError(f'Add a site rendering for the diagram in {source}')
        diagram = (ROOT / 'site/templates/fork-diagram.html').read_text()
        text = re.sub(r'```mermaid\n.*?```', lambda _: diagram, text, flags=re.S)
    lines, rust = [], False
    for line in text.splitlines():
        if line.startswith('```'):
            rust = line.startswith('```rust')
            if rust:
                line = '```rust'
        if rust and (line == '#' or line.startswith('# ')):
            continue
        lines.append(line)
    return '\n'.join(lines) + '\n'

shutil.copytree(ROOT / 'site/templates', STAGING / 'templates', dirs_exist_ok=True)
shutil.copytree(ROOT / 'site/static', STAGING / 'static', dirs_exist_ok=True)
shutil.copytree(ROOT / 'site/content', STAGING / 'content', dirs_exist_ok=True)
shutil.copy2(ROOT / 'site/zola.toml', STAGING / 'zola.toml')
for source, (slug, title) in PAGES.items():
    front = f'+++\ntitle = {json.dumps(title)}\ntemplate = "page.html"\n[extra]\nsource = {json.dumps(REPO + source)}\n+++\n\n'
    (STAGING / 'content' / (slug + '.md')).write_text(front + content(source))
trace = subprocess.check_output(['cargo', 'run', '--quiet', '-p', 'how-far-example-app', '--example', 'site_trace'], cwd=ROOT, text=True)
json.loads(trace)
(STAGING / 'static/traces.json').write_text(trace)
subprocess.run(['zola', '--root', str(STAGING), 'build', '--force', '--output-dir', str(OUTPUT)], check=True, cwd=ROOT)
print(f'Site built: {OUTPUT}')
