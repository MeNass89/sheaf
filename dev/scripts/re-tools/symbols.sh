#!/bin/zsh
# usage: symbols.sh <binary> <outdir> <label> [swift]
B="$1"; O="$2"; L="$3"; mkdir -p "$O"
strings -a -n 6 "$B" | LC_ALL=C grep -E '^[A-Z][A-Za-z ,'"'"'&()-]{4,70}$|Action|Tool|Annotation|Command|Menu|Toolbar|Panel|Sidebar|Inspector|Shortcut|Preference|Setting|Dialog|Error|[a-z][A-Za-z]+:$|^[a-z][A-Za-z]+(:[A-Za-z]*)+:$' \
  | LC_ALL=C grep -vE '^[[:space:]]*$|[^[:print:]]' | awk 'length($0)<=200' | awk '!s[$0]++' > "$O/$L.txt"
if [ "$4" = swift ]; then
  nm -gU "$B" 2>/dev/null | awk '{print $NF}' | xcrun swift-demangle --simplified 2>/dev/null \
   | LC_ALL=C grep -E 'PDF_Expert|RD[A-Z][A-Za-z]+\.|AIChat|HomeTab|PageNumbersDetector|ReaddleLib|Readdle|^_?[A-Z][A-Za-z]+(Action|Tool|Annotation|Command|Menu|Toolbar|Controller|View|Panel)' \
   | grep -vE 'outlined|metadata accessor|type metadata|nominal type descriptor|protocol conformance descriptor|reflection metadata|method descriptor|dispatch thunk|property descriptor|associated (type|conformance)|value witness|default argument|lazy cache|merged ' \
   | awk '!s[$0]++' | head -20000 > "$O/$L.swift-symbols.txt"
  [ -s "$O/$L.swift-symbols.txt" ] || rm -f "$O/$L.swift-symbols.txt"
fi
echo "$L $(wc -l < "$O/$L.txt") $( [ -f "$O/$L.swift-symbols.txt" ] && wc -l < "$O/$L.swift-symbols.txt")"
