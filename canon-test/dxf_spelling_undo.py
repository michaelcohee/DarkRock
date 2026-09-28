"""A2 reversible ASCII DXF normalization; spelling helpers are shared with future v2 edits."""
from __future__ import annotations
from decimal import Decimal,InvalidOperation
import hashlib,math,struct

MAGIC=b'DXA2U1'
TIMESTAMPS={b'$TDCREATE',b'$TDUPDATE',b'$TDINDWG',b'$TDUSRTIMER'}

def exact_decimal_token(token:bytes):
    """Exact decimal value for a future NUMBER_SPELLING_UNDO equality check."""
    try:
        value=Decimal(token.strip().decode('ascii'))
        return value if value.is_finite() else None
    except (InvalidOperation,UnicodeDecodeError):return None

def shortest_roundtrip_spelling(token:bytes)->bytes|None:
    """Shortest Python binary64 round-trip representation, preserving source via undo."""
    try:
        value=float(token.strip().decode('ascii'))
        if not math.isfinite(value):return None
        return repr(value).encode('ascii')
    except (ValueError,OverflowError,UnicodeDecodeError):return None

def numeric_value_code(code:int)->bool:
    return 10<=code<=59 or 110<=code<=149 or 210<=code<=239

def _lines(data:bytes)->list[bytes]:
    return data.splitlines(keepends=True)

def _clean(line:bytes)->bytes:
    return line.rstrip(b'\r\n').rstrip(b' \t')+b'\n'

def _tag_lines(data:bytes):
    if data.startswith(b'AutoCAD Binary DXF') or b'\0' in data:raise ValueError('binary DXF is unsupported')
    lines=_lines(data)
    if len(lines)%2:raise ValueError('odd ASCII DXF line count')
    tags=[]
    for i in range(0,len(lines),2):
        try:code=int(lines[i].strip())
        except ValueError as e:raise ValueError('invalid DXF group code') from e
        tags.append((code,lines[i],lines[i+1]))
    return tags

def encode_undo(original:bytes,ops:list[tuple[int,int,bytes]])->bytes:
    out=bytearray(MAGIC+struct.pack('<QI',len(original),len(ops))+hashlib.sha256(original).digest())
    for offset,new_len,old in ops:
        out+=struct.pack('<QII',offset,new_len,len(old))+old
    return bytes(out)

def restore(normalized:bytes,undo:bytes)->bytes:
    if not undo.startswith(MAGIC):raise ValueError('wrong undo magic')
    at=len(MAGIC)
    def take(n):
        nonlocal at
        value=undo[at:at+n]
        if len(value)!=n:raise ValueError('truncated undo')
        at+=n;return value
    original_len,count=struct.unpack('<QI',take(12));original_hash=take(32)
    ops=[]
    for _ in range(count):
        offset,new_len,old_len=struct.unpack('<QII',take(16));ops.append((offset,new_len,take(old_len)))
    if at!=len(undo):raise ValueError('trailing undo bytes')
    result=bytearray(normalized)
    for offset,new_len,old in reversed(ops):
        if offset+new_len>len(result):raise ValueError('invalid undo position')
        result[offset:offset+new_len]=old
    raw=bytes(result)
    if len(raw)!=original_len or hashlib.sha256(raw).digest()!=original_hash:raise ValueError('undo hash mismatch')
    return raw

def normalize(data:bytes)->tuple[bytes,bytes,dict]:
    tags=_tag_lines(data);out=bytearray();ops=[];section=b'';counts={'numeric_spelling':0,'line_format':0,'header_timestamp':0}
    i=0
    while i<len(tags):
        code,code_line,value_line=tags[i]
        value=value_line.strip()
        if code==0 and value==b'SECTION' and i+1<len(tags) and tags[i+1][0]==2:section=tags[i+1][2].strip()
        if code==0 and value==b'ENDSEC':section=b''
        if section==b'HEADER' and code==9 and value in TIMESTAMPS and i+1<len(tags) and tags[i+1][0] not in (0,9):
            removed=code_line+value_line+tags[i+1][1]+tags[i+1][2]
            ops.append((len(out),0,removed));counts['header_timestamp']+=1;i+=2;continue
        clean_code=_clean(code_line);clean_value=_clean(value_line)
        if numeric_value_code(code):
            short=shortest_roundtrip_spelling(value_line)
            if short is not None:
                clean_value=short+b'\n'
                if clean_value!=_clean(value_line):counts['numeric_spelling']+=1
        for old,new in ((code_line,clean_code),(value_line,clean_value)):
            if old!=new:
                ops.append((len(out),len(new),old))
                if new==_clean(old):counts['line_format']+=1
            out+=new
        i+=1
    normalized=bytes(out);undo=encode_undo(data,ops)
    if restore(normalized,undo)!=data:raise ValueError('A2 round-trip mismatch')
    counts['undo_ops']=len(ops);counts['undo_bytes']=len(undo)
    return normalized,undo,counts
