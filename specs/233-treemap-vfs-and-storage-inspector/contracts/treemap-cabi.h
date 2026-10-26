// Copyright 2026 MetastudyLine. All rights reserved.
// SPDX-License-Identifier: BSD-3-Clause

#ifndef TTWORKSPACE_TREEMAP_CABI_H
#define TTWORKSPACE_TREEMAP_CABI_H

#include <stdint.h>
#include <stddef.h>
#include <stdbool.h>

#ifdef __cplusplus
extern "C" {
#endif

/// 32-byte canonical C-ABI treemap tile layout.
typedef struct __attribute__((aligned(8))) {
    float x;             // 0..4:   Normalized [0.0..1.0] or pixel X coordinate
    float y;             // 4..8:   Normalized [0.0..1.0] or pixel Y coordinate
    float width;         // 8..12:  Tile width
    float height;        // 12..16: Tile height
    uint32_t node_id;    // 16..20: VFS CompactNode ID (0xFFFFFFFF = Others)
    uint16_t depth;      // 20..22: Relative directory depth
    uint8_t category;    // 22..23: TileCategory discriminant
    uint8_t flags;       // 23..24: TileFlags bitmask
    uint64_t bytes;      // 24..32: Total uncompressed / physical size
} TTWorkspaceTreemapTile;

#if defined(__STDC_VERSION__) && __STDC_VERSION__ >= 201112L
_Static_assert(sizeof(TTWorkspaceTreemapTile) == 32, "TTWorkspaceTreemapTile must be exactly 32 bytes");
_Static_assert(_Alignof(TTWorkspaceTreemapTile) == 8, "TTWorkspaceTreemapTile must be 8-byte aligned");
#endif

/// Compute treemap layout for the given VFS parent directory node.
/// Returns total tiles written into out_tiles, up to max_tiles.
int32_t ttworkspace_vfs_compute_treemap_layout(
    uintptr_t session_handle,
    uint32_t parent_id,
    float width_px,
    float height_px,
    uint16_t max_depth,
    TTWorkspaceTreemapTile *out_tiles,
    uint32_t max_tiles,
    uint32_t *out_tile_count
);

#ifdef __cplusplus
}
#endif

#endif // TTWORKSPACE_TREEMAP_CABI_H
