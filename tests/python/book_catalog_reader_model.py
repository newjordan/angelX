"""Prototype reader: verify a coherent source manifest; no fallback."""
import hashlib,json
from pathlib import Path
from publish import EXPECTED,SCHEMA,read_bounded,validate

def unique_pairs(pairs):
    result={}
    for key,value in pairs:
        if key in result: raise ValueError('duplicate manifest key')
        result[key]=value
    return result

def load(root, between=None):
    directory=Path(root)/'cockpit/book'
    first=read_bounded(directory/'catalog-manifest.json')
    manifest=json.loads(first,object_pairs_hook=unique_pairs)
    if set(manifest)!={'schema','chapters'} or manifest['schema']!=SCHEMA or set(manifest['chapters'])!=set(EXPECTED):
        raise ValueError('invalid catalog manifest')
    output={}
    for index,name in enumerate(EXPECTED):
        entry=manifest['chapters'][name]
        if set(entry)!={'cell','sha256'} or entry['cell']!=EXPECTED[name][0] or not isinstance(entry['sha256'],str) or len(entry['sha256'])!=64:
            raise ValueError('invalid chapter identity')
        data=read_bounded(directory/name)
        if hashlib.sha256(data).hexdigest()!=entry['sha256']: raise ValueError('unpublished or changed chapter')
        validate(name,data)
        output[name]=data
        if between: between(index,name)
    if read_bounded(directory/'catalog-manifest.json')!=first: raise ValueError('manifest changed while loading')
    return output
