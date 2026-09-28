import unittest
from dxf_spelling_undo import normalize,restore,exact_decimal_token,shortest_roundtrip_spelling

def dxf(body:bytes, eol=b'\n')->bytes:
    return (b'0\nSECTION\n2\nHEADER\n9\n$TDCREATE\n40\n2450000.000000\n9\n$INSUNITS\n70\n4\n0\nENDSEC\n0\nSECTION\n2\nENTITIES\n'+body+b'0\nENDSEC\n0\nEOF\n').replace(b'\n',eol)

class A2Tests(unittest.TestCase):
    def check(self,original):
        normalized,undo,counts=normalize(original)
        self.assertEqual(restore(normalized,undo),original)
        damaged=bytearray(undo);damaged[-1]^=1
        with self.assertRaises(ValueError):restore(normalized,bytes(damaged))
        return normalized,counts
    def test_spelling(self):
        a=dxf(b'0\nLINE\n10\n12.5\n20\n1e+01\n')
        b=dxf(b'0\nLINE\n10\n12.500000\n20\n10.0\n')
        x,_=self.check(a);y,_=self.check(b);self.assertEqual(x,y)
        self.assertEqual(exact_decimal_token(b'12.5'),exact_decimal_token(b'12.500000'))
        self.assertEqual(shortest_roundtrip_spelling(b'1e+01'),b'10.0')
    def test_signed_zero_and_exponents(self):
        a=dxf(b'0\nPOINT\n10\n-0.000\n20\n1.25e+002\n')
        n,_=self.check(a);self.assertIn(b'-0.0\n',n);self.assertIn(b'125.0\n',n)
    def test_crlf_and_trailing_spaces(self):
        a=dxf(b'0\nLINE\n10\n12.500000   \n',b'\r\n')
        n,_=self.check(a);self.assertNotIn(b'\r',n);self.assertNotIn(b'   ',n)
    def test_missing_header_var(self):
        a=b'0\nSECTION\n2\nHEADER\n9\n$INSUNITS\n70\n4\n0\nENDSEC\n0\nEOF\n'
        n,c=self.check(a);self.assertEqual(c['header_timestamp'],0);self.assertEqual(n,a)
    def test_binary_fails_closed(self):
        with self.assertRaises(ValueError):normalize(b'AutoCAD Binary DXF\r\n\x1a\0')

if __name__=='__main__':unittest.main()
