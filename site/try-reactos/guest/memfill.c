// MEMFILL.EXE: touch every free page of RAM once, then give it back, so the pages that held stale data hold zeros and a saved
// machine state compresses better. No C runtime. Build: guest/build-memfill.sh. Leaves 12 MB alone so the guest does not page out.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT
typedef unsigned int U;
typedef struct { U len, load, totalPhys, availPhys, totalPage, availPage, totalVirt, availVirt; } MEMSTAT;
__declspec(dllimport) void __stdcall GlobalMemoryStatus(MEMSTAT *);
__declspec(dllimport) void *__stdcall VirtualAlloc(void *, U, U, U);
__declspec(dllimport) int __stdcall VirtualFree(void *, U, U);
__declspec(dllimport) void __stdcall Sleep(U);
__declspec(dllimport) void __stdcall ExitProcess(U);

#define MB (1024u * 1024u)

void start(void)
{
	MEMSTAT m;
	m.len = sizeof m;
	GlobalMemoryStatus(&m);
	U want = m.availPhys > 12 * MB ? (m.availPhys - 12 * MB) / MB : 0;
	static void *chunk[512];
	U n = 0;
	while (n < want && n < 512) {
		unsigned char *p = VirtualAlloc(0, MB, 0x3000, 4);
		if (!p) break;
		for (U i = 0; i < MB; i += 4096) {
			p[i] = 1;
			p[i] = 0;
		}
		chunk[n++] = p;
	}
	Sleep(500);
	for (U i = 0; i < n; i++) VirtualFree(chunk[i], 0, 0x8000);
	ExitProcess(0);
}
