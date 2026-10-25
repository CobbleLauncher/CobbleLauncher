// SPDX-License-Identifier: MIT
// Host-owned fixed ABI; no caller-provided native code, paths, or callbacks.
#include "generator.h"
#include "finders.h"
#include "util.h"
#include <stdlib.h>

static int cobble_mc(int version) {
    switch (version) {
        case 11202: return MC_1_12_2;
        case 11601: return MC_1_16_1;
        case 11605: return MC_1_16_5;
        case 11701: return MC_1_17_1;
        case 11802: return MC_1_18_2;
        case 11902: return MC_1_19_2;
        case 11904: return MC_1_19_4;
        // Explicit family prediction: upstream represents all 1.20 generation
        // by MC_1_20 (alias MC_1_20_6), not a latest-version fallback.
        case 12001: return MC_1_20;
        case 12006: return MC_1_20_6;
        case 12101: return MC_1_21_1;
        case 12103: return MC_1_21_3;
        default: return MC_UNDEF;
    }
}
void *cobble_seed_new(uint64_t seed, int version, int dim, int large) {
    Generator *g = (Generator *)malloc(sizeof(Generator));
    if (!g) return NULL;
    setupGenerator(g, cobble_mc(version), large ? LARGE_BIOMES : 0);
    applySeed(g, dim, seed);
    return g;
}
void cobble_seed_free(void *g) { free(g); }
int cobble_seed_biome(const void *g, int x, int y, int z) {
    return getBiomeAt((const Generator *)g, 1, x, y, z);
}
const char *cobble_seed_biome_name(int id, int version) {
    return biome2str(cobble_mc(version), id);
}
int cobble_seed_biome_dimension(int id, int version) {
    int mc = cobble_mc(version);
    if (!biomeExists(mc, id) || id == the_void) return 1000;
    // biomeExists includes historical IDs that no longer generate; the
    // upstream isOverworld predicate supplies the generation filter.
    if (getDimension(id) == DIM_OVERWORLD && !isOverworld(mc, id)) return 1000;
    return getDimension(id);
}
unsigned int cobble_seed_biome_color(int id) {
    unsigned char colors[256][3];
    initBiomeColors(colors);
    if (id < 0 || id >= 256) return 0;
    return ((unsigned int)colors[id][0] << 16) |
           ((unsigned int)colors[id][1] << 8) | colors[id][2];
}
int cobble_seed_structure_config(int type, int version, int *dim) {
    StructureConfig conf;
    if (!getStructureConfig(type, cobble_mc(version), &conf)) return 0;
    *dim = conf.dim;
    return conf.regionSize * 16;
}
int cobble_seed_structure(void *ptr, int type, int rx, int rz, int *x, int *z) {
    Generator *g = (Generator *)ptr;
    Pos p;
    if (!getStructurePos(type, g->mc, g->seed, rx, rz, &p)) return 0;
    if (!isViableStructurePos(type, g, p.x, p.z, 0)) return 0;
    *x = p.x; *z = p.z;
    return 1;
}
