/*
 * Thin C ABI for the Rust side of `emusic-sid`.
 *
 * cRSID's `cRSID_generateSample` is declared `static inline` (so Rust cannot
 * link against it directly), and the upstream build expects an SDL host.
 * This translation unit includes the library source and exposes a few
 * non-static wrappers instead.
 *
 * `CRSID_PLATFORM_PC` is deliberately left undefined so the SDL code in
 * `host/audio.c` and the file-loading helper in `host/file.c` are compiled
 * out; only the portable emulation is used.
 */

#include "../vendor/crsid/libcRSID.c"

/* Creates the (single, process-global) cRSID instance. Returns NULL on
 * failure, mirroring `cRSID_init`. */
void* emusic_crsid_init(unsigned short samplerate) {
    return (void*) cRSID_init(samplerate, 0);
}

/* Copies `size` bytes of `data` into C64 memory and returns a pointer to the
 * SID header inside that same buffer (NULL if the magic is wrong). */
void* emusic_crsid_process(void* c64, unsigned char* data, int size) {
    return (void*) cRSID_processSIDfile((cRSID_C64instance*)c64, data, size);
}

/* (Re)initialises `subtune` (1-based). */
void emusic_crsid_init_tune(void* c64, void* header, int subtune) {
    cRSID_initSIDtune((cRSID_C64instance*)c64, (cRSID_SIDheader*)header,
                      (char)subtune);
}

/* Renders `count` mono signed 16-bit samples. */
void emusic_crsid_render(void* c64, short* out, int count) {
    cRSID_C64instance* instance = (cRSID_C64instance*)c64;
    int i;
    for (i = 0; i < count; i++) {
        out[i] = cRSID_generateSample(instance);
    }
}

/* Rewrites the header's SID-model / video-standard bits so the next
 * `cRSID_initSIDtune` (which calls `cRSID_setC64`) picks them up.
 *
 * chip_model: 6581 or 8580 to force a model, anything else to keep the file's.
 * clock:      0 = NTSC, 1 = PAL, anything else to keep the file's.
 *
 * Only PSID/RSID v2+ headers have these fields; for v1 files the bytes are
 * tune data, so the override is skipped.
 */
void emusic_crsid_override(void* c64, int chip_model, int clock) {
    cRSID_C64instance* instance = (cRSID_C64instance*)c64;
    cRSID_SIDheader* header = instance->SIDheader;
    if (header == 0 || header->Version < 2) return;

    if (chip_model == 6581 || chip_model == 8580) {
        unsigned char bits = (chip_model == 8580) ? 0x20 : 0x10;
        header->ModelFormatStandard =
            (unsigned char)((header->ModelFormatStandard & ~0x30) | bits);
    }
    if (clock == 0 || clock == 1) {
        unsigned char bits = clock ? 0x04 : 0x08;
        header->ModelFormatStandard =
            (unsigned char)((header->ModelFormatStandard & ~0x0C) | bits);
    }
}
