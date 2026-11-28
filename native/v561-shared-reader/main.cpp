// Diagnostic helper: read-only mapping, separate binary media pipe; no output authority.
#define WIN32_LEAN_AND_MEAN
#include <windows.h>
#include <fcntl.h>
#include <io.h>
#include <cstdint>
#include <iostream>
#include <limits>
#include <string>
#include <vector>

int wmain(int argc, wchar_t **argv) {
    if (argc != 3) return 2;
    std::size_t used = 0;
    unsigned long long size;
    try { size = std::stoull(argv[2], &used); } catch (...) { return 2; }
    if (used != std::wstring(argv[2]).size() || size == 0 || size > 268435456ULL) return 2;
    HANDLE map = OpenFileMappingW(FILE_MAP_READ, FALSE, argv[1]);
    if (!map) return 3;
    auto *view = static_cast<const std::uint8_t *>(MapViewOfFile(map, FILE_MAP_READ, 0, 0, static_cast<SIZE_T>(size)));
    if (!view) { CloseHandle(map); return 4; }
    _setmode(_fileno(stdin), _O_BINARY);
    _setmode(_fileno(stdout), _O_BINARY);
    std::uint64_t request[2];
    while (std::cin.read(reinterpret_cast<char *>(request), sizeof(request))) {
        const auto offset = request[0], length = request[1];
        if (length == 0 || offset > size || length > size - offset || length > 16777216ULL) {
            UnmapViewOfFile(view); CloseHandle(map); return 5;
        }
        std::cout.write(reinterpret_cast<const char *>(view + offset), static_cast<std::streamsize>(length));
        std::cout.flush();
        if (!std::cout) { UnmapViewOfFile(view); CloseHandle(map); return 6; }
    }
    UnmapViewOfFile(view);
    CloseHandle(map);
    return std::cin.eof() ? 0 : 7;
}
