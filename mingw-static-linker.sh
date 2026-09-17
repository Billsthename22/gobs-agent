#!/bin/bash

STATIC_STDCXX="$(x86_64-w64-mingw32-gcc -print-file-name=libstdc++.a)"

ARGS=()

for arg in "$@"; do
    if [ "$arg" = "-lstdc++" ]; then
        ARGS+=("$STATIC_STDCXX")
    else
        ARGS+=("$arg")
    fi
done

exec x86_64-w64-mingw32-gcc "${ARGS[@]}"
