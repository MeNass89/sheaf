"""Sheaf sample plugin: writes <output> as <input> with an incremental update setting /Title (Reviewed).
Sheaf runs it as `python3 mark_reviewed.py <input.pdf> <output.pdf>` in this folder; anything printed on
stderr is shown when it fails."""
import re
import sys

src, dst = sys.argv[1], sys.argv[2]
data = open(src, "rb").read()
try:
    prev = int(re.findall(rb"startxref\s+(\d+)", data)[-1])
    root = re.findall(rb"/Root\s+(\d+\s+\d+\s+R)", data)[-1].decode()
    size = int(re.findall(rb"/Size\s+(\d+)", data)[-1])
except IndexError:
    sys.exit("not a PDF this sample can update (no trailer found)")
if not data.endswith(b"\n"):
    data += b"\n"
offset = len(data)
obj = f"{size} 0 obj\n<< /Title (Reviewed) /Producer (Sheaf sample plugin) >>\nendobj\n".encode()
xref_at = offset + len(obj)
tail = (f"xref\n{size} 1\n{offset:010d} 00000 n \ntrailer\n<< /Size {size + 1} /Root {root} /Info {size} 0 R /Prev {prev} >>\n"
        f"startxref\n{xref_at}\n%%EOF\n").encode()
open(dst, "wb").write(data + obj + tail)
