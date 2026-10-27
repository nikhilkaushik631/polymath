"""Copy license texts from embedded asset trees while preserving their provenance paths."""
from pathlib import Path
import shutil,sys
root=Path(__file__).resolve().parent.parent.parent
out=Path(sys.argv[1])
for name in ('reader','writer','sheets','slides','lightcraft'):
    repo=root/name
    for tree in ('assets','vendor','docs/brand','crates/fonts'):
        base=repo/tree
        if not base.exists():continue
        for p in base.rglob('*'):
            if p.is_file() and any(term in p.name.upper() for term in ('LICENSE','LICENCE','NOTICE','COPYING','OFL','ATTRIBUTION')) and p.stat().st_size<2_000_000:
                target=out/name/p.relative_to(repo);target.parent.mkdir(parents=True,exist_ok=True);shutil.copyfile(p,target)

# Font and PDF compatibility data bundled through Cargo carry their own license texts.
import os,tomllib
suite=Path(__file__).resolve().parent.parent
lock=tomllib.loads((suite/'Cargo.lock').read_text())
registry=Path(os.environ.get('CARGO_HOME',str(Path.home()/'.cargo')))/'registry/src'
required={
 'epaint_default_fonts':['fonts/UFL.txt','fonts/OFL.txt','fonts/Hack-Regular.txt','fonts/emoji-icon-font-mit-license.txt'],
 'hayro-cmap':['assets/LICENSE.txt','LICENSE-MIT','LICENSE-APACHE'],
}
for name,files in required.items():
    version=next(package['version'] for package in lock['package'] if package['name']==name)
    sources=list(registry.glob(f'*/{name}-{version}'))
    if not sources:raise SystemExit(f'Missing cached license sources for {name}-{version}; build the application before bundling.')
    for relative in files:
        source=sources[0]/relative
        target=out/'bundled-dependencies'/f'{name}-{version}'/relative
        target.parent.mkdir(parents=True,exist_ok=True);shutil.copyfile(source,target)
