#!/bin/zsh
# all symbols (incl. local) demangled + ObjC classes/selectors + _Tt strings; app-specific
B="$1"; OUT="$2"; PAT="$3"
{
 nm -a "$B" 2>/dev/null | awk '{print $NF}'
 strings -a -n 6 "$B" | grep -E '^_?Tt[CVOP]|^\$s|^_\$s'
} | awk '!s[$0]++' | xcrun swift-demangle --simplified 2>/dev/null \
 | LC_ALL=C grep -E "$PAT" \
 | grep -vE 'outlined|metadata accessor|type metadata|nominal type descriptor|protocol conformance descriptor|reflection metadata|method descriptor|dispatch thunk|property descriptor|associated (type|conformance)|value witness|default argument|lazy cache|merged |partial apply|reabstraction|closure #|implicit closure|block_copy|block_destroy|objc_msgSend\$|^ltmp|^l_|^__' \
 | sed -E 's/^_OBJC_(CLASS|METACLASS)_\$_//' | awk '!s[$0]++' | head -20000 > "$OUT"
wc -l < "$OUT"
