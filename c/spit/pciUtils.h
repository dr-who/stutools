#ifndef __PCIUTILS_H
#define __PCIUTILS_H

// 0x0100 == storage
// 0x0200 == networking
// 0x0300 == video
// 0x0400 == audio
// 0x0c00 == USB
void listPCIdevices(size_t classfilter);

#endif


