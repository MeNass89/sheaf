#!/bin/zsh
# usage: types.sh <binary> <out>
B="$1"; OUT="$2"
{
echo "## ObjC/Swift class names (__objc_classname, demangled)"
python3 -I /tmp/sheaf-extract/machosec.py "$B" __objc_classname | sed 's/^\[__objc_classname\] //' | xcrun swift-demangle --simplified | awk '!s[$0]++'
echo; echo "## Swift type names from __swift5_typeref/mangled strings"
strings -a -n 6 "$B" | grep -E '^_?Tt[CVOP]' | xcrun swift-demangle --simplified | awk '!s[$0]++'
echo; echo "## ObjC selectors (__objc_methname)"
python3 -I /tmp/sheaf-extract/machosec.py "$B" __objc_methname | sed 's/^\[__objc_methname\] //' | awk '!s[$0]++'
echo; echo "## Swift stored property names (__swift5_reflstr)"
python3 -I /tmp/sheaf-extract/machosec.py "$B" __swift5_reflstr | sed 's/^\[__swift5_reflstr\] //' | awk '!s[$0]++'
} | head -20000 > "$OUT"
wc -l < "$OUT"
