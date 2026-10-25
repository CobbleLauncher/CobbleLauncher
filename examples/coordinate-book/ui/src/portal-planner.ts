// SPDX-License-Identifier: MIT
import type { Dimension, Location } from './model';

export type PortalDirection = 'overworld-to-nether' | 'nether-to-overworld';
export type PortalCandidate = {
  location: Location;
  projected: { x: number; z: number };
  dx: number;
  dz: number;
  horizontalOffset: number;
  yDelta: number | null;
};

const targetDimension = (direction: PortalDirection): Dimension => direction === 'overworld-to-nether' ? 'nether' : 'overworld';
export function portalCandidates(locations: Location[], originId: string, direction: PortalDirection): PortalCandidate[] {
  const origin = locations.find(location => location.id === originId);
  const fromDimension = direction === 'overworld-to-nether' ? 'overworld' : 'nether';
  if (!origin || origin.dimension !== fromDimension) return [];
  const projected = direction === 'overworld-to-nether'
    ? { x: Math.floor(origin.x / 8), z: Math.floor(origin.z / 8) }
    : { x: origin.x * 8, z: origin.z * 8 };
  return locations.filter(location => location.dimension === targetDimension(direction)).map(location => {
    const dx = location.x - projected.x, dz = location.z - projected.z;
    return { location, projected, dx, dz, horizontalOffset: Math.hypot(dx, dz), yDelta: origin.y === null || location.y === null ? null : location.y - origin.y };
  }).sort((a, b) => a.horizontalOffset - b.horizontalOffset || a.location.name.localeCompare(b.location.name));
}

export function portalOriginLocations(locations: Location[], direction: PortalDirection): Location[] {
  const dimension = direction === 'overworld-to-nether' ? 'overworld' : 'nether';
  return locations.filter(location => location.dimension === dimension);
}

