"""Generate small synthetic privacy fixtures; no real personal information."""
import base64, io, json, struct, zlib, zipfile
from pathlib import Path
fixtures = {}
def add(name, data): fixtures[name] = base64.b64encode(data).decode()
def chunk(kind, value):
    return struct.pack('>I',len(value))+kind+value+struct.pack('>I',zlib.crc32(kind+value))
png = b'\x89PNG\r\n\x1a\n'+chunk(b'IHDR',struct.pack('>IIBBBBB',1,1,8,2,0,0,0))+chunk(b'tEXt',b'Author\0PRIVATE_CANARY')+chunk(b'IDAT',zlib.compress(b'\0\xff\0\0'))+chunk(b'IEND',b'')
add('image.png',png)
# Minimal valid baseline JPEG and WebP (generated fixtures are decoded separately).
# Browser/native parity for these is covered by the module corpus and optional fixtures.
for ext,main,mime in [('docx','word/document.xml','wordprocessingml.document'),('xlsx','xl/workbook.xml','spreadsheetml.sheet'),('pptx','ppt/presentation.xml','presentationml.presentation')]:
    output = io.BytesIO()
    with zipfile.ZipFile(output,'w',zipfile.ZIP_STORED) as archive:
        archive.writestr('[Content_Types].xml',f'<Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types"><Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/><Default Extension="xml" ContentType="application/xml"/><Override PartName="/{main}" ContentType="application/vnd.openxmlformats-officedocument.{mime}.main+xml"/><Override PartName="/docProps/core.xml" ContentType="application/vnd.openxmlformats-package.core-properties+xml"/></Types>')
        archive.writestr('_rels/.rels',f'<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="r1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="{main}"/><Relationship Id="r2" Type="http://schemas.openxmlformats.org/package/2006/relationships/metadata/core-properties" Target="docProps/core.xml"/></Relationships>')
        archive.writestr(main,'<document>visible content</document>')
        archive.writestr('docProps/core.xml','<properties>PRIVATE_CANARY</properties>')
        archive.comment=b'PRIVATE_CANARY'
    add('document.'+ext,output.getvalue())
objects=[b'<< /Type /Catalog /Pages 2 0 R >>',b'<< /Type /Pages /Kids [3 0 R] /Count 1 >>',b'<< /Type /Page /Parent 2 0 R /MediaBox [0 0 100 100] >>',b'<< /Author (PRIVATE_CANARY) >>']
pdf=b'%PDF-1.4\n'; offsets=[]
for i,obj in enumerate(objects,1):
    offsets.append(len(pdf));pdf+=f'{i} 0 obj\n'.encode()+obj+b'\nendobj\n'
xref=len(pdf);pdf+=b'xref\n0 5\n0000000000 65535 f \n'
for offset in offsets:pdf+=f'{offset:010d} 00000 n \n'.encode()
pdf+=f'trailer\n<< /Size 5 /Root 1 0 R /Info 4 0 R >>\nstartxref\n{xref}\n%%EOF\n'.encode()
add('document.pdf',pdf)
Path(__file__).with_name('privacy.json').write_text(json.dumps(fixtures,indent=2)+'\n')
