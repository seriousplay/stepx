"""Compatibility fix for the hash-pinned loader; never changes the Google payload."""
import re

IMAGE_BASE = '''def image_base(name):
  # A library may also be mapped as a whole read-only file. That is not its ELF load bias.
  images=[l.split() for l in maps if l.endswith('/'+name) and 'x' in l.split()[1]]
  bases={int(l[0].split('-')[0],16)-int(l[2],16) for l in images}
  if len(bases)!=1:raise RuntimeError('No unique executable native image: '+name)
  return bases.pop()
'''

def adapt(source):
    source,count=re.subn(r'def image_base\(name\):\n.*?(?=for soname,expected)',IMAGE_BASE,source,flags=re.S)
    if count!=1:raise RuntimeError('Native loader image-base template changed')
    marker="cb=image_base('libc.so');ab=image_base('libart.so')"
    validation='''
for address in (cb+0x79170,cb+0xd77c0,ab+0x669a4c):
 if not any('x' in l.split()[1] and int(l.split()[0].split('-')[0],16)<=address<int(l.split()[0].split('-')[1],16) for l in maps):
  raise RuntimeError('Native entry point is not executable; stopped before attaching')
'''
    if source.count(marker)!=1:raise RuntimeError('Native entry point template changed')
    return source.replace(marker,marker+validation)
