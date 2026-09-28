# Подключается перед project() в whisper.cpp (через CMAKE_PROJECT_INCLUDE_BEFORE из .cargo/config.toml).
#
# По умолчанию ggml собирается «под текущий процессор» (GGML_NATIVE): на CI это серверный Xeon
# с AVX-512 — такой exe падает на домашних CPU. А при кросс-сборке наоборот выключает всё до SSE4.2,
# и распознавание ползёт. Фиксируем базу, которая есть у любого x86-64 процессора с 2013 года:
# AVX2 + FMA + F16C.
set(GGML_NATIVE OFF CACHE BOOL "" FORCE)
set(GGML_AVX ON CACHE BOOL "" FORCE)
set(GGML_AVX2 ON CACHE BOOL "" FORCE)
set(GGML_FMA ON CACHE BOOL "" FORCE)
set(GGML_F16C ON CACHE BOOL "" FORCE)
set(GGML_AVX512 OFF CACHE BOOL "" FORCE)
set(GGML_AVX512_VBMI OFF CACHE BOOL "" FORCE)
set(GGML_AVX512_VNNI OFF CACHE BOOL "" FORCE)
set(GGML_AVX512_BF16 OFF CACHE BOOL "" FORCE)
set(GGML_AVX_VNNI OFF CACHE BOOL "" FORCE)
set(GGML_AMX_TILE OFF CACHE BOOL "" FORCE)
set(GGML_AMX_INT8 OFF CACHE BOOL "" FORCE)
set(GGML_AMX_BF16 OFF CACHE BOOL "" FORCE)
