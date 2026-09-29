// Browsey ABI probe; SPDX-License-Identifier: MIT
#include <cstddef>
#include "vendor/unrar/rar.hpp"

extern "C" std::size_t browsey_unrar_abi(unsigned int index) {
  switch (index) {
    case 0: return sizeof(RARHeaderData);
    case 1: return sizeof(RARHeaderDataEx);
    case 2: return sizeof(RAROpenArchiveData);
    case 3: return sizeof(RAROpenArchiveDataEx);
    case 4: return offsetof(RARHeaderDataEx, CmtBuf);
    case 5: return offsetof(RARHeaderDataEx, RedirType);
    case 6: return offsetof(RARHeaderDataEx, ArcNameEx);
    case 7: return offsetof(RAROpenArchiveDataEx, Callback);
    case 8: return offsetof(RAROpenArchiveDataEx, CmtBufW);
    case 9: return offsetof(RAROpenArchiveDataEx, MarkOfTheWeb);
    case 10: return RARVER_MAJOR;
    case 11: return RARVER_MINOR;
    case 12: return RARVER_BETA;
    default: return static_cast<std::size_t>(-1);
  }
}
